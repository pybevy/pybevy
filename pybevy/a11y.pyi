"""Accessibility plugin for PyBevy."""

from pybevy.app import App, Plugin
from pybevy.ecs import Component

class Role:
    """Accessibility role for UI elements.

    The members are class attributes that hold shared `Role` instances.
    """

    def __copy__(self) -> Role: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Role: ...


    # Common interactive elements
    Unknown: Role
    TextRun: Role
    Cell: Role
    Label: Role
    Image: Role
    Link: Role
    Row: Role
    ListItem: Role
    ListMarker: Role
    TreeItem: Role
    ListBoxOption: Role
    MenuItem: Role
    MenuListOption: Role
    Paragraph: Role
    GenericContainer: Role
    CheckBox: Role
    RadioButton: Role
    TextInput: Role
    Button: Role
    DefaultButton: Role
    Pane: Role
    RowHeader: Role
    ColumnHeader: Role
    RowGroup: Role
    List: Role
    Table: Role
    LayoutTableCell: Role
    LayoutTableRow: Role
    LayoutTable: Role
    Switch: Role
    Menu: Role
    MultilineTextInput: Role
    SearchInput: Role
    DateInput: Role
    DateTimeInput: Role
    WeekInput: Role
    MonthInput: Role
    TimeInput: Role
    EmailInput: Role
    NumberInput: Role
    PasswordInput: Role
    PhoneNumberInput: Role
    UrlInput: Role
    Abbr: Role
    Alert: Role
    AlertDialog: Role
    Application: Role
    Article: Role
    Audio: Role
    Banner: Role
    Blockquote: Role
    Canvas: Role
    Caption: Role
    Caret: Role
    Code: Role
    ColorWell: Role
    ComboBox: Role
    EditableComboBox: Role
    Complementary: Role
    Comment: Role
    ContentDeletion: Role
    ContentInsertion: Role
    ContentInfo: Role
    Definition: Role
    DescriptionList: Role
    Details: Role
    Dialog: Role
    DisclosureTriangle: Role
    Document: Role
    EmbeddedObject: Role
    Emphasis: Role
    Feed: Role
    FigureCaption: Role
    Figure: Role
    Footer: Role
    Form: Role
    Grid: Role
    GridCell: Role
    Group: Role
    Header: Role
    Heading: Role
    Iframe: Role
    IframePresentational: Role
    ImeCandidate: Role
    Keyboard: Role
    Legend: Role
    LineBreak: Role
    ListBox: Role
    Log: Role
    Main: Role
    Mark: Role
    Marquee: Role
    Math: Role
    MenuBar: Role
    MenuItemCheckBox: Role
    MenuItemRadio: Role
    MenuListPopup: Role
    Meter: Role
    Navigation: Role
    Note: Role
    PluginObject: Role
    ProgressIndicator: Role
    RadioGroup: Role
    Region: Role
    RootWebArea: Role
    Ruby: Role
    RubyAnnotation: Role
    ScrollBar: Role
    ScrollView: Role
    Search: Role
    Section: Role
    SectionFooter: Role
    SectionHeader: Role
    Slider: Role
    SpinButton: Role
    Splitter: Role
    Status: Role
    Strong: Role
    Suggestion: Role
    SvgRoot: Role
    Tab: Role
    TabList: Role
    TabPanel: Role
    Term: Role
    Time: Role
    Timer: Role
    TitleBar: Role
    Toolbar: Role
    Tooltip: Role
    Tree: Role
    TreeGrid: Role
    Video: Role
    WebView: Role
    Window: Role
    PdfActionableHighlight: Role
    PdfRoot: Role
    GraphicsDocument: Role
    GraphicsObject: Role
    GraphicsSymbol: Role
    DocAbstract: Role
    DocAcknowledgements: Role
    DocAfterword: Role
    DocAppendix: Role
    DocBackLink: Role
    DocBiblioEntry: Role
    DocBibliography: Role
    DocBiblioRef: Role
    DocChapter: Role
    DocColophon: Role
    DocConclusion: Role
    DocCover: Role
    DocCredit: Role
    DocCredits: Role
    DocDedication: Role
    DocEndnote: Role
    DocEndnotes: Role
    DocEpigraph: Role
    DocEpilogue: Role
    DocErrata: Role
    DocExample: Role
    DocFootnote: Role
    DocForeword: Role
    DocGlossary: Role
    DocGlossRef: Role
    DocIndex: Role
    DocIntroduction: Role
    DocNoteRef: Role
    DocNotice: Role
    DocPageBreak: Role
    DocPageFooter: Role
    DocPageHeader: Role
    DocPageList: Role
    DocPart: Role
    DocPreface: Role
    DocPrologue: Role
    DocPullquote: Role
    DocQna: Role
    DocSubtitle: Role
    DocTip: Role
    DocToc: Role
    ListGrid: Role
    Terminal: Role

    def __hash__(self) -> int: ...

class AccessibilityNode(Component):
    """Component for accessibility integration using AccessKit.

    Represents an entity in the accessibility tree, allowing screen readers
    and other assistive technologies to interact with UI elements.

    Example:
        ```python
        # Create an accessibility node for a button
        commands.spawn((
            Button(),
            AccessibilityNode(Role.Button),
        ))

        # Create a list item with a label
        node = AccessibilityNode(Role.ListItem)
        node.set_label("Item 1")
        commands.spawn((Text("Item 1"), node))
        ```
    """

    def __init__(self, role: Role = Role.Unknown) -> None: ...

    @property
    def role(self) -> Role:
        """The accessibility role of this node."""

    def set_role(self, role: Role) -> None:
        """Set the accessibility role."""

    @property
    def label(self) -> str | None:
        """The accessible label (name) of this node."""

    def set_label(self, label: str) -> None:
        """Set the accessible label."""

    def clear_label(self) -> None:
        """Clear the accessible label."""

    @property
    def value(self) -> str | None:
        """The current value of this node (for inputs, sliders, etc.)."""

    def set_value(self, value: str) -> None:
        """Set the current value."""

    def clear_value(self) -> None:
        """Clear the current value."""

    @property
    def description(self) -> str | None:
        """Additional descriptive text about this node."""

    def set_description(self, description: str) -> None:
        """Set the description."""

    def clear_description(self) -> None:
        """Clear the description."""

    @property
    def is_disabled(self) -> bool:
        """Whether this node is disabled."""

    def set_disabled(self, disabled: bool) -> None:
        """Set the disabled state."""

    @property
    def is_hidden(self) -> bool:
        """Whether this node is hidden from assistive technology."""

    def set_hidden(self, hidden: bool) -> None:
        """Set the hidden state."""

    @property
    def is_expanded(self) -> bool | None:
        """Whether this node is expanded (for expandable elements)."""

    def set_expanded(self, expanded: bool) -> None:
        """Set the expanded state."""

    def clear_expanded(self) -> None:
        """Clear the expanded state."""

    @property
    def is_selected(self) -> bool | None:
        """Whether this node is selected."""

    def set_selected(self, selected: bool) -> None:
        """Set the selected state."""

    def clear_selected(self) -> None:
        """Clear the selected state."""

    @property
    def numeric_value(self) -> float | None:
        """The numeric value (for sliders, progress bars, etc.)."""

    def set_numeric_value(self, value: float) -> None:
        """Set the numeric value."""

    def clear_numeric_value(self) -> None:
        """Clear the numeric value."""

    @property
    def min_numeric_value(self) -> float | None:
        """The minimum numeric value."""

    def set_min_numeric_value(self, value: float) -> None:
        """Set the minimum numeric value."""

    def clear_min_numeric_value(self) -> None:
        """Clear the minimum numeric value."""

    @property
    def max_numeric_value(self) -> float | None:
        """The maximum numeric value."""

    def set_max_numeric_value(self, value: float) -> None:
        """Set the maximum numeric value."""

    def clear_max_numeric_value(self) -> None:
        """Clear the maximum numeric value."""

    def set_bounds(
        self, min_x: float, min_y: float, max_x: float, max_y: float
    ) -> None:
        """Set the bounding rectangle for this node in screen coordinates."""

    def clear_bounds(self) -> None:
        """Clear the bounding rectangle."""

class AccessibilityPlugin(Plugin):
    """Accessibility plugin providing screen reader and accessibility support."""

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
