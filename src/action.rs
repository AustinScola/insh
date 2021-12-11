#[derive(PartialEq)]
pub enum Action {
    Exit,
    RunBash,

    EnterBrowseMode,
    EnterFindMode,
    EnterBrowseFindMode,
    EnterSearchMode,
    EnterBrowseSearchMode,

    // Browse mode actions.
    BrowseScrollDown,
    BrowseScrollUp,
    BrowseDrillDown,
    BrowseDrillUp,
    BrowseEdit,

    // Find mode actions.
    FindScrollDown,
    FindScrollUp,
    FindDeletePreviousCharacter,
    FindAppendCharacter(char),
    FindBrowseSelectedParent,
    FindEditFile,

    // Search mode actions.
    SearchScrollDown,
    SearchScrollUp,
    SearchEditFile,
    SearchAppendCharacter(char),
    SearchDeletePreviousCharacter,
}
