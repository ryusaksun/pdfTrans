# AGENTS.md

## Project Overview
PDFMathTranslate is a Python-based PDF translation tool for scientific papers, built on BabelDOC.
It supports multiple translation engines, GUI, and CLI interfaces.

## Build / Lint / Test Commands

### Running Tests
- Run all tests: `uv run pytest .`
- Run specific test file: `pytest tests/config/test_main.py`
- Run specific test: `pytest tests/config/test_main.py::TestConfigManager::test_singleton`

### Linting
- Check and auto-fix: `uv run ruff --fix`
- Format code: `uv run ruff-format`
- Pre-commit hook (if installed): `pre-commit run --all-files`

### Building
- Build package: `uv build`

## Code Style Guidelines

### Python Version
- Minimum: Python 3.10
- Maximum: Python 3.13
- Target version for ruff: py310

### Imports
- Use `from __future__ import annotations` at top of files for forward references
- Single-line imports enforced by isort (force-single-line)
- Use pathlib instead of os.path (Ruff PTH rule)
- Group imports: stdlib → third-party → local (Ruff I rule)
- Example:
  ```python
  from __future__ import annotations

  import logging
  from pathlib import Path

  import httpx
  import openai
  from pydantic import BaseModel

  from pdf2zh_next.config.model import SettingsModel
  ```

### Formatting (Ruff)
- Max line length: 88 (ruff) / 120 (flake8)
- Ruff ignores: E203, E261, E501, E741, F841, S101, SIM, ARG002, B024, etc.
- Double quotes for docstrings, single quotes for code (flake8-quotes config)
- Trailing commas omitted (COM812 ignored)

### Type Hints
- Required: All functions and classes should have type hints
- Use `|` union syntax (Python 3.10+) instead of `typing.Union`
- Example: `str | None` not `Optional[str]`
- Pydantic models for all configuration structures
- Example:
  ```python
  class BasicSettings(BaseModel):
      input_files: set[str] = Field(default=set(), description="...")
      debug: bool = Field(default=False, description="...")
  ```

### Naming Conventions
- Classes: PascalCase (e.g., `BaseTranslator`, `SettingsModel`)
- Functions/Variables: snake_case (e.g., `do_translate`, `lang_in`)
- Constants: UPPER_SNAKE_CASE (e.g., `__version__`, `DEFAULT_CONFIG_DIR`)
- Private members: single underscore prefix (e.g., `_read_toml_file`)
- Protected members: single underscore prefix (ruff N rule enforcement)

### Documentation
- Google-style docstrings (Ruff pydocstyle convention)
- Class/method/docstring format:
  ```python
  class BaseTranslator(ABC):
      """Base class for all translators"""

      def translate(self, text: str) -> str:
          """Translate text with caching.

          Args:
              text: Text to translate

          Returns:
              Translated text
          """
          ...
  ```

### Error Handling
- Use custom exception classes for domain errors (see `high_level.py`)
- `contextlib.suppress` for safe cleanup (no empty try-except)
- Graceful degradation: log and continue for non-critical failures
- Example:
  ```python
  def __del__(self):
      with contextlib.suppress(Exception):
          logger.info(f"{self.name} stats...")
  ```

### Architecture Patterns
- Abstract base classes: Use `ABC` with `@abstractmethod` (see `BaseTranslator`)
- Pydantic settings: All config models extend `BaseModel`
- Singleton pattern: Use for ConfigManager
- Translation cache: Implemented with TranslationCache and peewee
- Rate limiting: Use BaseRateLimiter with tenacity retry decorators
- Async: Use `asyncio` and collections.abc for type hints

### Field Restrictions in Pydantic Models
IMPORTANT: Only these Field parameters are allowed in `pdf2zh_next/config/model.py`:
- `default`
- `description`
- `default_factory`
- `alias`
- `discriminator`

For other Field parameters, add forwarding in `pdf2zh_next/config/cli_env_model.py`
at `__cli_env_settings_model_fields`.

### Testing
- Pytest framework
- Fixtures in `tests/config/conftest.py`
- Test naming: `test_<function_name>` in `TestClassName` classes
- Use `tmp_path` fixture for temporary files
- Use `monkeypatch` fixture for environment/config mocking

### Logging
- Standard `logging` module with `RichHandler` for output
- Logger per module: `logger = logging.getLogger(__name__)`
- Structured logging with appropriate levels (DEBUG, INFO, WARNING, ERROR, CRITICAL)
- Use `logger.debug()` for cache misses and recoverable errors
- Use `logger.critical()` for critical issues (e.g., unimplemented methods)

### Version Management
- Version defined in: `pdf2zh_next/__init__.py`, `pdf2zh_next/const.py`
- Use bumpver for version bumps (configured in pyproject.toml)
- Version sync: update `pyproject.toml`, `__init__.py`, `const.py`, `main.py`

### Configuration Priority
CLI args > Environment vars > User config file > Default config
Environment variables: `PDF2ZH_*` prefix (e.g., `PDF2ZH_DEBUG=true`)
Config dir: `~/.config/pdf2zh/`
Config files: `config.v{version}.toml`

### Important Notes
- Translator names must be ≤20 characters (cache.py constraint)
- Always validate settings before use (`validate_settings()` method)
- Use `SettingsModel.model_json_schema()` for CLI argument generation
- Avoid suppressing type errors (no `as any`, `@ts-ignore`)
- Use tenacity for retry logic with exponential backoff
