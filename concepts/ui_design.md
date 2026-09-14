# UI design notes

Written by AI from design decisions discussed with the project author.

## Colors

- **Primary:** signals clickable actions. Every clickable control should have a primary-colored cue; informational text should not use primary.
- **Green:** success.
- **Yellow / amber:** warning.
- **Red:** error or danger, including destructive actions. This is the semantic exception to primary action styling.

Use the theme's primary color and the existing shared status colors consistently. Pair status colors with meaningful text or icons so color is not the only indication.

## Lists and navigation

- Lists use alternating row backgrounds.
- Clickable rows remain visually quiet: the whole row is clickable, with a primary-colored chevron at the far right. The chevron is not a separate button.
- The button owns striping and hover/pressed overlays; an optional container background supplies a status tint.
- Odd and even rows share a hover style that stands out from both resting backgrounds. Status tints remain visible through the overlays.
- Keep row heights consistent within each list, with configurable pixel heights for different uses.

## Information and actions

Keep names, addresses, and other facts neutral unless they convey status. Put navigation and operations in explicit action buttons, such as **Device** and **Close** at the bottom of a tunnel card. Reserve list summaries for the information needed to decide what to open; show further detail in the drawer or detail view.
