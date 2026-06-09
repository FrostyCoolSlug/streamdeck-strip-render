# Stream Deck Strip Renderer

This is a rust lib for rendering stream deck layouts into images. The incoming layout must be complete and include
all the values for the fields, it's the callers responsibility to manage the layout.

**NOTE**: This is not perfect and will likely need small adjustments over time when plugins are found that do
other stuff.

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
- It might make sense to render some widgets at 2x then resize to introduce some aliasing
- The Stream Deck app ignores widgets that render past the edge of the canvas, I just clip them.
- The canvas is currently black by default, but this may need to be transparent
- This should probably have some logging :D

### Text
- Currently includes an embedded font to solve headaches
- Overflow Fade is not yet implemented
- If the value is missing, we currently render `{{key}}`

### Pixmaps
- Missing or unable to process pixmaps are rendered as checkerboards
- Images currently scale to fit the rect
- Images don't maintain aspect ratio when resizing

### Bar
- Subtypes 2 & 3 (Trapezoid) not implemented, will render as groove
- I have no idea how DoubleTrapezoid or DoubleRectangle work from a value perspective
- Bars currently stretch to fit the rect, but the example renders don't do that.

### GBar
- All Bar related issues apply here too
- The arrow looks terrible, might need aliasing
- Are borders on the arrows forced?
