# scremind

A Windows screen overlay reminder application.

### Configuration

The application reads configuration from `config.toml`. If the file doesn't exist, a default one will be created automatically.

```toml
[overlay]
fade_duration = 2
hold_duration = [1, 5]  # [min, max] seconds to hold
fps = 60

[foreground]
font_size = 120
font_name = "Arial"
fg_color = [255, 255, 255, 150]  # [r, g, b, a]

[interval_reminder]
interval = 1800
bg_color = [19, 65, 52, 150]  # [r, g, b, a]

[[schedule_reminder]]
time = "11:58"
bg_color = [255, 95, 95, 180]  # [r, g, b, a]

[[schedule_reminder]]
time = "18:25"
bg_color = [0, 135, 205, 180]  # [r, g, b, a]
```

### Options

| Section | Option | Description | Default |
|---------|--------|-------------|---------|
| `overlay` | `fade_duration` | Fade animation duration in seconds | `1.0` |
| `overlay` | `hold_duration` | Hold time range as `[min, max]` in seconds | `[1.0, 5.0]` |
| `overlay` | `fps` | Animation frames per second | `60` |
| `foreground` | `font_size` | Reminder text font size | `72` |
| `foreground` | `font_name` | Font family name | `"Arial"` |
| `foreground` | `fg_color` | Text color as `[r, g, b, a]` array | `[255, 255, 255, 150]` |
| `interval_reminder` | `interval` | Reminder interval in seconds | `1800` (30 min) |
| `interval_reminder` | `bg_color` | Overlay background color as `[r, g, b, a]` array | `[255, 255, 255, 30]` |
| `schedule_reminder` | `time` | Scheduled time in `HH:MM` format | - |
| `schedule_reminder` | `bg_color` | Overlay background color as `[r, g, b, a]` array | `[255, 255, 255, 30]` |
