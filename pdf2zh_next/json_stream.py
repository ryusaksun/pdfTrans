"""JSON Lines streaming bridge for external TUI/GUI consumers.

Protocol:
  - stdout: one JSON object per line (events from Python to consumer)
  - stderr: human-readable log output
  - stdin:  one JSON object per line (commands from consumer to Python)

Events (Python → consumer):
  {"type": "ready", "version": "...", "engines": [...]}
  {"type": "config_schema", "engines": [...], "languages": {...}}
  {"type": "stage_summary", ...}
  {"type": "progress_start|progress_update|progress_end", ...}
  {"type": "finish", "translate_result": {...}, "token_usage": {...}}
  {"type": "error", "error": "...", "error_type": "...", "details": "..."}

Commands (consumer → Python):
  {"cmd": "translate", "settings": {...}, "files": [...]}
  {"cmd": "cancel"}
  {"cmd": "shutdown"}
"""

from __future__ import annotations

import asyncio
import json
import logging
import sys
import typing
from pathlib import Path

from pdf2zh_next.config.model import SettingsModel
from pdf2zh_next.config.translate_engine_model import (
    GUI_PASSWORD_FIELDS,
    GUI_SENSITIVE_FIELDS,
    TRANSLATION_ENGINE_METADATA,
)
from pdf2zh_next.high_level import TranslationError, do_translate_async_stream

logger = logging.getLogger(__name__)

# Language map (shared with gui.py but defined here to avoid importing gradio)
LANG_MAP = {
    "English": "en",
    "Simplified Chinese": "zh-CN",
    "Traditional Chinese - Hong Kong": "zh-HK",
    "Traditional Chinese - Taiwan": "zh-TW",
    "Japanese": "ja",
    "Korean": "ko",
    "Polish": "pl",
    "Russian": "ru",
    "Spanish": "es",
    "Portuguese": "pt",
    "Brazilian Portuguese": "pt-BR",
    "French": "fr",
    "Malay": "ms",
    "Indonesian": "id",
    "Vietnamese": "vi",
    "German": "de",
    "Dutch": "nl",
    "Italian": "it",
    "Greek": "el",
    "Swedish": "sv",
    "Danish": "da",
    "Norwegian": "no",
    "Finnish": "fi",
    "Ukrainian": "uk",
    "Czech": "cs",
    "Romanian": "ro",
    "Hungarian": "hu",
    "Slovak": "sk",
    "Croatian": "hr",
    "Estonian": "et",
    "Latvian": "lv",
    "Lithuanian": "lt",
    "Bulgarian": "bg",
    "Serbian (Cyrillic)": "sr",
    "Slovenian": "sl",
    "Catalan": "ca",
    "Turkish": "tr",
    "Thai": "th",
    "Arabic": "ar",
    "Hindi": "hi",
    "Bengali": "bn",
    "Urdu": "ur",
    "Persian": "fa",
    "Hebrew": "he",
    "Swahili": "sw",
    "Afrikaans": "af",
    "Icelandic": "is",
    "Irish": "ga",
    "Albanian": "sq",
    "Macedonian": "mk",
    "Belarusian": "be",
    "Georgian": "ka",
    "Armenian": "hy",
    "Mongolian (Cyrillic)": "mn",
    "Khmer": "km",
    "Lao": "lo",
    "Burmese": "my",
    "Tamil": "ta",
    "Telugu": "te",
    "Malayalam": "ml",
    "Gujarati": "gu",
    "Sinhala": "si",
    "Bosnian": "bs",
    "Filipino (Tagalog)": "tl",
    "Haitian Creole": "ht",
    "Latin": "la",
    "Uzbek": "uz",
    "Kazakh (Latin)": "kk",
    "Kyrgyz": "ky",
    "Tajik": "tg",
    "Turkmen": "tk",
    "Luxembourgish": "lb",
    "Maltese": "mt",
    "Amharic": "am",
}


def _emit(obj: dict) -> None:
    """Write a single JSON line to stdout and flush."""
    sys.stdout.write(json.dumps(obj, ensure_ascii=False, default=str) + "\n")
    sys.stdout.flush()


def _build_engine_schema() -> list[dict]:
    """Build engine metadata for the TUI's dynamic forms."""
    engines = []
    for metadata in TRANSLATION_ENGINE_METADATA:
        fields = []
        for field_name, field in metadata.setting_model_type.model_fields.items():
            if field_name in ("translate_engine_type", "support_llm"):
                continue
            if field.default_factory:
                continue

            type_hint = field.annotation
            type_args = typing.get_args(type_hint)
            if type_hint is bool or bool in type_args:
                field_type = "bool"
            elif type_hint is int or int in type_args:
                field_type = "int"
            else:
                field_type = "str"

            fields.append({
                "name": field_name,
                "type": field_type,
                "default": field.default,
                "description": field.description or "",
                "sensitive": field_name in GUI_SENSITIVE_FIELDS,
                "password": field_name in GUI_PASSWORD_FIELDS,
            })

        engines.append({
            "name": metadata.translate_engine_type,
            "support_llm": metadata.support_llm,
            "cli_flag": metadata.cli_flag_name,
            "fields": fields,
        })
    return engines


def _serialize_event(event: dict) -> dict:
    """Serialize an event dict, converting non-serializable objects."""
    out = {}
    for k, v in event.items():
        if v is None or isinstance(v, (str, int, float, bool, list, dict)):
            out[k] = v
        elif hasattr(v, "__dict__"):
            # Convert objects like translate_result to dicts
            out[k] = {
                attr: getattr(v, attr)
                for attr in vars(v)
                if not attr.startswith("_")
            }
        else:
            out[k] = str(v)
    return out


async def _do_translate(settings_dict: dict, files: list[str]) -> None:
    """Run translation for given files, emitting events to stdout."""
    for file_path in files:
        try:
            settings = SettingsModel(**settings_dict)
            async for event in do_translate_async_stream(settings, Path(file_path)):
                _emit(_serialize_event(event))
                if event["type"] in ("finish", "error"):
                    break
        except TranslationError as e:
            _emit({
                "type": "error",
                "error": str(e),
                "error_type": e.__class__.__name__,
                "details": getattr(e, "original_error", "")
                or getattr(e, "traceback_str", "")
                or "",
            })
        except asyncio.CancelledError:
            _emit({
                "type": "error",
                "error": "Translation cancelled",
                "error_type": "CancelledError",
                "details": "",
            })
            return
        except Exception as e:
            _emit({
                "type": "error",
                "error": str(e),
                "error_type": type(e).__name__,
                "details": "",
            })


async def run_json_stream(settings: SettingsModel) -> None:
    """Main loop: read commands from stdin, write events to stdout."""
    from pdf2zh_next.main import __version__

    # Redirect all logging to stderr so stdout stays clean for JSON
    root_logger = logging.getLogger()
    root_logger.handlers.clear()
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(logging.Formatter("%(levelname)s:%(name)s:%(message)s"))
    root_logger.addHandler(handler)

    # Emit ready event
    engine_names = [m.translate_engine_type for m in TRANSLATION_ENGINE_METADATA]
    _emit({"type": "ready", "version": __version__, "engines": engine_names})

    # Emit config schema
    _emit({
        "type": "config_schema",
        "engines": _build_engine_schema(),
        "languages": LANG_MAP,
    })

    # Read commands from stdin
    loop = asyncio.get_event_loop()
    reader = asyncio.StreamReader()
    protocol = asyncio.StreamReaderProtocol(reader)
    await loop.connect_read_pipe(lambda: protocol, sys.stdin)

    current_task: asyncio.Task | None = None

    while True:
        line = await reader.readline()
        if not line:
            break
        try:
            cmd = json.loads(line.decode().strip())
        except json.JSONDecodeError:
            continue

        cmd_type = cmd.get("cmd")

        if cmd_type == "shutdown":
            if current_task and not current_task.done():
                current_task.cancel()
                try:
                    await current_task
                except (asyncio.CancelledError, Exception):
                    pass
            break

        elif cmd_type == "cancel":
            if current_task and not current_task.done():
                current_task.cancel()
                try:
                    await current_task
                except (asyncio.CancelledError, Exception):
                    pass
                current_task = None

        elif cmd_type == "translate":
            settings_dict = cmd.get("settings", {})
            files = cmd.get("files", [])
            if current_task and not current_task.done():
                current_task.cancel()
                try:
                    await current_task
                except (asyncio.CancelledError, Exception):
                    pass
            current_task = asyncio.create_task(_do_translate(settings_dict, files))

        elif cmd_type == "validate":
            settings_dict = cmd.get("settings", {})
            try:
                s = SettingsModel(**settings_dict)
                s.validate_settings()
                _emit({"type": "validation_result", "valid": True, "error": None})
            except Exception as e:
                _emit({"type": "validation_result", "valid": False, "error": str(e)})
