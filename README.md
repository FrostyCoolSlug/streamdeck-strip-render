# Stream Deck Strip Renderer

This is a rust lib for rendering stream deck layouts into images. The incoming layout must be complete and include
all the values for the fields, it's the callers responsibility to manage the layout.

**NOTE**: This is not perfect and will likely need small adjustments over time when plugins are found that do
other stuff.

## Feature Flags

| Feature            | Description                                                                                      |
|--------------------|--------------------------------------------------------------------------------------------------|
| `strict-rendering` | Rejects entire layouts on z-index overlaps. Ignores layout items which exceed the canvas size. |
| `super-sample`     | Renders at 4× resolution and downsamples for improved visual quality.                            |

*Note:* The `super-sample` feature will disable itself in debug builds. In debug mode, image rendering can exceed 150ms,
whereas release builds are typically closer to ~10ms. The delay can give the appearance of a laggy UI on the deck.

## Tests

`cargo test` will find any json files in the `test-data` directory and attempt to render them. The 'final' images
will be placed in `target/test-output/`, so for quick layout testing, you can drop it in and run.

## Notes

Most of the problems below are simply due to the lack of official documentation, there aren't a lot of useful
presentation examples, so I've been making guesses here and there. These will be solved when I make a deck plugin and
can confirm how they're supposed to render.

### General

- Most examples don't present the same way they should render
    - I'm wondering whether things have internal paddings / margins that aren't documented
- The canvas is currently black by default, but this may need to be transparent

### Text

- Currently includes an embedded font to solve headaches
- Overflow Fade is not yet implemented
- If the value is missing, we currently render `{{key}}`

### Pixmaps

- Missing or unable to process pixmaps are rendered as checkerboards
- Images currently scale to fit the rect
- Images don't maintain aspect ratio when resizing