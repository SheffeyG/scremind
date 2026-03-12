# scremind

A Windows screen overlay reminder application.

## Configuration

The application reads configuration from `config.toml`. If the file doesn't exist, a default one will be created automatically.


<details>
<summary>Full Configuration Example</summary>

```toml
[overlay]
fade_duration = 2
fps = 60

[foreground]
font_size = 120
font_name = "Arial"

[foreground.font_color]
r = 255
g = 255
b = 255
a = 150

[interval_reminder]
interval = 1800

[interval_reminder.color]
r = 19
g = 65
b = 52
a = 120

[[scheduled_reminders]]
time = "11:58"

[scheduled_reminders.color]
r = 255
g = 95
b = 95
a = 120

[[scheduled_reminders]]
time = "18:25"

[scheduled_reminders.color]
r = 0
g = 135
b = 205
a = 120
```

</details>

## Configuration Options

| Section | Option | Description | Default |
|---------|--------|-------------|---------|
| `overlay` | `fade_duration` | Fade animation duration in seconds | `1.0` |
| `overlay` | `fps` | Animation frames per second | `60` |
| `foreground` | `font_size` | Reminder text font size | `72` |
| `foreground` | `font_name` | Font family name | `"Arial"` |
| `foreground.font_color` | `r`, `g`, `b`, `a` | Text color (RGBA, 0-255) | `255, 255, 255, 30` |
| `interval_reminder` | `interval` | Reminder interval in seconds | `1800` (30 min) |
| `interval_reminder.color` | `r`, `g`, `b`, `a` | Overlay color (RGBA, 0-255) | `255, 255, 255, 30` |
| `scheduled_reminders` | `time` | Scheduled time in `HH:MM` format | - |
| `scheduled_reminders.color` | `r`, `g`, `b`, `a` | Overlay color for this reminder | `255, 255, 255, 30` |
