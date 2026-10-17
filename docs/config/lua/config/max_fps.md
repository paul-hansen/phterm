---
tags:
  - tuning
---
# `max_fps = 165`

Limits the maximum number of frames per second that wezterm will
attempt to draw.

Defaults to `165`. {{since('nightly', inline=True)}} Previously the
default was `60`, which added up to ~16ms of keypress-to-screen latency
because the echoed character had to wait out the throttle interval
started by the keypress repaint.

| Environment | Supported Since |
|-------------|-----------------|
| Wayland     | Ignored; instead, uses information from the compositor to schedule painting frames |
| X11         | {{since('20211204-082213-a66c61ee9', inline=True)}} |
| macOS       | {{since('20220903-194523-3bb1ed61', inline=True)}} |
| Windows     | {{since('20220903-194523-3bb1ed61', inline=True)}} |

