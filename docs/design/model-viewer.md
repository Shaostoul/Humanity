# The 3D model viewer

**Asked for:** 2026-10-16, by the operator, after adding a `.glb` to a Media item: "can you make it so
people can view the 3D model in-app and web?" The Media design (files-showcase.md, "The page") had
promised "the model viewer"; until now a model offered only Download.

Native first, the website mirrors it, both from this one spec: the same files, words, camera,
controls, lighting and limits. Desktop: `src/gui/model_view/` (an egui paint callback drawing the
model with the app's GPU). Website: `web/shared/model-view.js` (WebGL 2, no outside library).

## What it opens

- **`.glb`** (binary glTF 2.0): meshes on their nodes (each node's matrix or translation, rotation
  and scale applied, through its parents), triangle primitives, the metallic-roughness material
  (base colour factor and texture, metallic and roughness factors and texture, normal texture,
  occlusion texture, emissive factor and texture), alpha mode (OPAQUE, MASK with its cutoff, BLEND)
  and double-sided. Embedded PNG and JPEG images. The scene named by `scene`, else the first, else
  every root node. Static pose: skins, morph targets and animations are not played.
- **`.stl`**, binary or ASCII. STL is z-up (3D printing), so it is turned to y-up (x stays, y = z,
  z = -y). Normals are worked out from each triangle's corners, never trusted from the file (often
  zero). One plain material: base colour (0.72, 0.74, 0.78), metallic 0, roughness 0.55, both
  sides drawn.
- Each texture is read as its file says (its glTF sampler): `magFilter` NEAREST or LINEAR,
  `minFilter` NEAREST, LINEAR or one of the four mipmap modes, `wrapS`/`wrapT` CLAMP_TO_EDGE,
  MIRRORED_REPEAT or REPEAT. Where the file gives no sampler, or leaves a field out: LINEAR,
  LINEAR_MIPMAP_LINEAR, REPEAT. Mipmaps are always made, so every mode works. (A value outside
  glTF's set: the website uses the default; the desktop's glTF reader refuses the file.)
- Not opened, said plainly: Draco-compressed meshes (`KHR_draco_mesh_compression`), points and
  lines, a `.gltf` with outside files, `.blend`, `.obj`, `.fbx` and everything else. These files
  keep Download.

## Where it appears

On an item's page, each file whose name ends in `.glb` or `.stl` gets **View in 3D** beside
Download. Nothing is fetched until it is pressed (bandwidth, and a model can be large). Pressed, the
viewer takes the place of the cover at the top of the page, with the file's name and **Close the 3D
view**. Not offered, and an open view closes, where the protected setup hides this person's pictures
(a model's textures are pictures too): the desktop's `tiles::show_of` Hidden rule, which the website
mirrors in `media-item.js` `hiddenBy`. On the website nobody counts as a friend there (Chat's friend
list is in its encrypted store, which the Media page cannot open), so it errs towards hiding. The
file comes from the same address the page already has for it (`ItemFile.url`: the
public `/uploads/<name>`, or for a members-only item the members' file route with this session's
upload token, exactly as pictures are fetched). Viewing does not count as a download.

## Words (both apps, exactly)

- Button: `View in 3D`. Tooltip: `Look at it here and turn it with the mouse. It is fetched from the server first ({size}).`
- Fetching: `Fetching {name} ({size})...` Reading: `Reading the model...`
- Shown, under the view: `{name}: drag to turn it, scroll to zoom, right-drag or Shift-drag to move it, double-click to start over. {triangles} triangles.` (triangles with thousands separators: `12,345`)
- Close: `Close the 3D view`
- Too large: `This model is {size}, larger than the 200 MB the viewer opens. Download it to see it.`
- Too detailed: `This model has {triangles} triangles, more than the 5,000,000 the viewer draws. Download it to see it.`
- Draco (required, or used and the mesh exists only compressed): `This model is compressed with Draco, which the viewer cannot read yet. Download it to see it in Blender or another 3D program.`
- Too many vertices: `This model has {vertices} vertices, more than the 15,000,000 the viewer draws. Download it to see it.`
- Nodes that share children over and over: `This 3D file could not be read: its nodes repeat each other too many times.`
- A fetch that did not work: `This model could not be fetched: {reason}.`
- Anything else unreadable: `This 3D file could not be read: {reason}.`
- No GPU drawing (an in-world screen, a test): `The 3D view needs the app's graphics; Download the file to see it.`

## Limits

- At most **200 MB** fetched (`MAX_MODEL_BYTES = 200 * 1024 * 1024`), checked against the size the
  page already knows before fetching, and again while reading.
- At most **5,000,000 triangles** (`MAX_TRIANGLES`) and **15,000,000 vertices** (`MAX_VERTICES`),
  counted before anything is read.
- At most **1,000,000 node visits** and 64 levels of nesting (a node graph that loops, or shares
  children over and over, stops there).
- On the desktop a model is uploaded in chunks of at most 4,194,304 vertices (128 MiB, half wgpu's
  default buffer limit; `load::CHUNK_VERTICES`), split on the loading thread, whole triangles only.
- Textures larger than **4096** on a side are scaled down to fit 4096.
- Every index, offset, count and stride from the file is bounds-checked: a broken or hostile file is
  an error message, never a crash or a read outside the file.

## Camera and controls

- Perspective, 35 degrees vertical field of view.
- Fit: the bounding sphere of every vertex after its nodes (centre `c`, radius `r`, at least 1e-6).
  Distance `d = r / sin(fov / 2) * 1.15`. Start: yaw 35 degrees, pitch 20 degrees. The camera sits at
  `c + d * (cos(pitch) sin(yaw), sin(pitch), cos(pitch) cos(yaw))`, looking at `c`, up +y.
- Drag (primary button): yaw -= dx * 0.008, pitch += dy * 0.008 (radians per point), pitch held to
  plus or minus 89 degrees.
- Right-drag, or Shift with the primary button: pan; the target moves along the camera's right and
  up by the drag times `2 * d * tan(fov / 2) / view height in points`.
- Scroll: `d *= exp(-scroll_points * 0.0015)`, held to `[0.05 r, 50 r]`.
- Double-click: back to the fit.
- Near `max(d - 2r, d * 0.001)`, far `d + 2r` (so zooming in never clips the model).
- No turning by itself: the model stays still until the person moves it.

## Lighting and shading

Physically based, glTF's own model: GGX distribution, Smith-Schlick geometry, Schlick Fresnel with
`F0 = mix(0.04, base, metallic)`, Lambert diffuse times `(1 - metallic)`. Roughness at least 0.04.
Base colour and emissive textures are sRGB; metallic-roughness (blue = metallic, green = roughness),
normal and occlusion textures are linear. Normal maps use a tangent frame worked out per pixel from
screen-space derivatives (no tangent attribute needed). Back faces of double-sided materials flip
their normal.

A studio setup that keeps the model lit whichever way it is turned. Three lights move with the
camera (directions in view space, towards the light):

| light | direction (view space) | colour | intensity |
| --- | --- | --- | --- |
| key | (-0.45, 0.60, 0.66) | (1.00, 0.97, 0.92) | 2.6 |
| fill | (0.70, 0.15, 0.50) | (0.85, 0.90, 1.00) | 0.8 |
| rim | (0.00, 0.45, -0.89) | (1.00, 1.00, 1.00) | 1.1 |

Plus a hemisphere in world space: sky (0.55, 0.60, 0.70) above, ground (0.25, 0.22, 0.20) below,
times 0.6, mixed by `n.y * 0.5 + 0.5`, lighting the diffuse colour plus `F0 * 0.25` for the
specular, times occlusion. Emissive is added. Then exposure 1.0 and the ACES filmic curve
(Narkowicz's fit), and the result is written as sRGB. The background is the theme's card colour.

Alpha: OPAQUE draws with alpha 1; MASK discards below its cutoff (default 0.5); BLEND draws after the
opaque parts, blended, without writing depth. Single-sided materials hide their back faces.

## Cost

Drawn only when something changed (the camera, the view's size, a new model); otherwise the last
picture is shown again. Mipmaps and chunks are made on the loading thread, never while drawing. 4x multisampling. One model in GPU memory per open view, released when the
view closes or has not been drawn for ten seconds. Nothing is networked beyond the one fetch.

## Tests

`scripts/tests/fixtures/model-cube.glb`, `model-wedge.stl` and `model-wedge-ascii.stl`, made by
`node scripts/gen-model-fixtures.js`: a textured cube on two nodes (its corners land at x 1..5, y and
z -2..2 once the nodes apply), and a two-triangle wedge in both STL forms. Both apps' tests read the
same files.

## Where the two apps differ, on purpose

- The website uploads positions relative to the model's centre (its WebGL path has no 64-bit step
  before the GPU); the desktop keeps world positions. They look the same at ordinary sizes.
- The website blends see-through parts after the colour is turned into sRGB (WebGL's default
  canvas), the desktop before (an sRGB target). See-through parts may look slightly different.
- If the website's 4x multisampled copy is refused, it draws straight to the canvas without
  multisampling.
