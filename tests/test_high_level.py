from pdf2zh_next.high_level import BabeldocError
from pdf2zh_next.high_level import SubprocessError
from pdf2zh_next.high_level import _is_scanned_pdf_error


def test_is_scanned_pdf_error_matches_babeldoc_and_subprocess_wrappers():
    assert _is_scanned_pdf_error(
        BabeldocError(
            "Babeldoc translation error: Scanned PDF detected.",
            original_error="Scanned PDF detected.",
        )
    )
    assert _is_scanned_pdf_error(
        SubprocessError(
            "Error during translation process: Scanned PDF detected.",
            traceback_str="babeldoc.babeldoc_exception.BabelDOCException.ScannedPDFError",
        )
    )


def test_is_scanned_pdf_error_ignores_unrelated_errors():
    assert not _is_scanned_pdf_error(BabeldocError("timeout", original_error="timeout"))
