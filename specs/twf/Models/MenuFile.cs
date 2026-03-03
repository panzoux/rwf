namespace TWF.Models;

/// <summary>
/// Represents a menu file containing a list of menu items.
/// </summary>
public class MenuFile
{
    /// <summary>
    /// Gets or sets the version of the menu file format.
    /// </summary>
    public string Version { get; set; } = "1.0";

    /// <summary>
    /// Gets or sets the list of menu items.
    /// </summary>
    public List<MenuItemDefinition> Menus { get; set; } = new();
}

/// <summary>
/// Represents a single menu item definition.
/// </summary>
public class MenuItemDefinition
{
    /// <summary>
    /// Gets or sets the display name of the menu item.
    /// </summary>
    public string Name { get; set; } = "";

    /// <summary>
    /// Gets or sets the custom function name to execute when selected.
    /// </summary>
    public string? Function { get; set; }

    /// <summary>
    /// Gets or sets the built-in action name to execute when selected.
    /// </summary>
    public string? Action { get; set; }

    /// <summary>
    /// Gets or sets the built-in action name (legacy property name, use Action instead).
    /// </summary>
    [Obsolete("Use Action property instead")]
    public string? Menu { get; set; }

    /// <summary>
    /// Gets a value indicating whether this menu item is a separator.
    /// </summary>
    public bool IsSeparator => Name == "-----";

    /// <summary>
    /// Gets a value indicating whether this menu item is selectable.
    /// A menu item is selectable if it's not a separator and has either a Function or Action property.
    /// </summary>
#pragma warning disable CS0618 // Type or member is obsolete
    public bool IsSelectable => !IsSeparator && (!string.IsNullOrEmpty(Function) || !string.IsNullOrEmpty(Action) || !string.IsNullOrEmpty(Menu));
#pragma warning restore CS0618 // Type or member is obsolete
}
