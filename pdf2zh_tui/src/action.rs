use crate::python_bridge::PythonEvent;

/// All possible application actions.
#[derive(Debug, Clone)]
pub enum Action {
    // Navigation
    SwitchPanel,
    FocusNext,
    FocusPrev,

    // Input
    CharInput(char),
    Backspace,
    EnterField,
    ExitField,

    // Dropdown
    OpenDropdown,
    CloseDropdown,
    SelectDropdownItem(usize),
    DropdownUp,
    DropdownDown,

    // Translation lifecycle
    StartTranslation,
    CancelTranslation,
    PythonEvent(PythonEvent),

    // Config
    SaveConfig,

    // File management
    AddFile(String),
    RemoveFile(usize),
    DeleteFocusedFile,

    // Screen transitions
    BackToConfigure,

    // System
    Resize(u16, u16),
    Tick,
    Quit,
    ShowError(String),
    DismissPopup,
    ShowHelp,

    // No-op
    None,
}
