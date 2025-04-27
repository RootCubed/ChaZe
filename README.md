# OSM Toolkit

OSM Toolkit is a command-line tool for rendering maps from OpenStreetMap PBF files using customizable style files. It generates high-quality PNG images based on the provided OSM data and style configuration.

## Features

- Parses OSM PBF files and renders map data
- Highly customizable rendering via style files (see `examples/`)
- Outputs a PNG image (`output.png` by default)

## Example Output

<img src="examples/output.png" height="600">

## Usage

### 1. Build the Project

Open a terminal in the project directory and run:

```pwsh
cargo build --release
```

### 2. Run the Renderer

To render a map, provide an OSM PBF file and a style file. The default output file is `output.png`.

```pwsh
.target\release\osm_toolkit.exe --input <your.osm.pbf> --style-file <your_style.chz>
```

Or specify an output file name:

```pwsh
.target\release\osm_toolkit.exe --input <your.osm.pbf> --style-file <your_style.chz> --output <output.png>
```

## Style Files

Style files define how different map features are rendered. See some examples in the `examples/` directory.

### Header

A style file begins with a header that specifies the output format, DPI, scale, and center coordinates. For example:

```
FORMAT 297 420
DPI 300
SCALE 1:50000
CENTER 47.39 8.68
```

### Layer definitions

An arbitrary number of layers can be defined after the header. The layers are rendered in the order they are defined, with the first layer being rendered first. Each layer has a name and a set of filters and rendering rules.

Define a layer by specifying its name in [square brackets]. For example:

```
[Streets]
```

After the layer name, you can specify filters to include or exclude the OSM features that should be rendered in that layer.

### Layer filters

At the start of a layer, the _Draw Set_ (the set of OSM features to be rendered) contains all OSM features in the input file. Filters allow you to reduce this set to only the features you want to render in that layer.

There are three types of filters: `@keep`, `@remove` and `@take`.

A `@keep` filter removes everything from the draw set that does not match the filter.

A `@remove` filter removes everything from the draw set that matches the filter.

A `@take` filter limits the number of features in the draw set to a specified number.

After specifying the name of the filter, you specify what you want to filter by. This is done by specifying a key and a value. The key is the name of the OSM tag you want to filter by, and the value is the value of that tag you want to keep or remove.

For example, to keep residential roads, you would use:

```
@keep highway="residential"
```

You can also use wildcards in the value. For example, to keep all roads:

```
@keep highway="*"
```

#### The `@sub` command

OSM relations are features that themselves contain other features. For example, a relation can contain a set of ways that form a bus route. The `@sub` command allows you to render the members of a relation instead of rendering the relation itself.

Here is an example of using the `@sub` command to render the stop points of a train route:

```
[Stations]
    @keep public_transport="stop_area" or public_transport="stop_position"
    @keep .relation
    @sub {
        @keep .role="stop"
        @keep train="yes" or light_rail="yes"
        @take 1
        Dot {
            radius: 1.3,
            color: #000000
        }
        Dot {
            radius: 1,
            color: #ffffff
        }
        Text {
            font_family: "Neue Frutiger World",
            size: 1.6,
            color: #000000,
            field: "name"
        }
    }
```

Note the usage of the special tags `.relation` and `.role`. These do not refer to OSM tags, but to meta information about the feature itself. The `.relation` tag is used to filter for relations (there are also `.node` and `.way`). The `.role` tag is used to filter for the role of a member in a relation.

### Rendering rules

Rendering rules specify how selected map features are drawn. Each rule is a command with parameters in curly braces. The following rendering commands are supported:

---

#### Polyfill

Fills polygons (ways or relations) with a solid color and optional transparency.

```
Polyfill {
    color: #RRGGBB | @random_color, // Fill color (hex or random)
    alpha: float                    // Opacity (0.0–1.0, default: 1.0)
}
```

- `color`: Hex color (e.g., `#f9f0d2`) or `@random_color` for random colors per feature.
- `alpha`: Opacity (default 1.0).

---

#### Outline

Draws outlines for ways or relations.

```
Outline {
    color: #RRGGBB | @random_color, // Stroke color
    width: float,                   // Line width (in mm, default: 1.0)
    alpha: float,                   // Opacity (default: 1.0)
    dash: "n1,n2,..."               // Optional dash pattern (comma-separated floats)
}
```

- `color`: Hex color or `@random_color`.
- `width`: Line width (multiplied by DPI scaling).
- `alpha`: Opacity.
- `dash`: Dash pattern (e.g., `"2,2"` for dashed lines).

---

#### Dot

Draws a dot at the location of a node, or at the centroid of a way or relation.

```
Dot {
    color: #RRGGBB | @random_color, // Dot color
    radius: float                   // Dot radius (in mm, default: 1.0)
}
```

- `color`: Hex color or `@random_color`.
- `radius`: Dot radius (multiplied by DPI scaling).

---

#### Text

Draws text labels for features. The label position is the node location, centroid of a way, or centroid of a relation.

```
Text {
    field: string,        // Tag key to use for label (default: "name")
    size: float,          // Font size (in mm, default: 12.0)
    color: #RRGGBB,       // Text color (default: black)
    font_family: string   // Font family (optional)
}
```

- `field`: Tag key to display (e.g., `name`).
- `size`: Font size (multiplied by DPI scaling).
- `color`: Text color.
- `font_family`: Font family (e.g., `Arial`).

Text rendering also supports advanced patching via `@patch_text` in the style file for per-feature label adjustments (see `examples/zh_station_map.chz` for an example usage).

---

### Example layer definition

```
[Water]
    @keep natural="water"
    Polyfill {
        color: #97b0f6,
        alpha: 1
    }
    Outline {
        color: #4060a0,
        width: 0.5,
        alpha: 0.8
    }
```

---

See the `examples/` directory for more style file samples and advanced usage.
