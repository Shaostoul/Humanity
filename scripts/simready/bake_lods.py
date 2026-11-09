# bake_lods.py - simplify, unwrap and bake one SimReady asset in Blender (docs/design/simready-props.md,
# section 4, step 3). Runs inside Blender (bpy):
#
#   blender --background --factory-startup --python-exit-code 1 --threads N \
#       --python scripts/simready/bake_lods.py -- <asset_dir> <build_dir> <config_json>
#
# The standard high-to-low game asset bake: the full-detail model as NVIDIA made it (every part,
# every material, its own normal maps) is baked onto a simplified copy with one fresh,
# non-overlapping UV set, so the shipped model is ONE mesh with ONE material (what the engine's
# machine-model path draws) and the detail that simplification removed survives as a normal map.
#
# Steps:
#   1. import the root layer with Blender's USD importer, UsdPreviewSurface materials only (the MDL
#      shaders are never read), textures referenced in place;
#   2. LOD1: a joined copy of every render mesh, simplified to the size class's triangle cap
#      (collapse), shaded smooth with sharp edges over 30 degrees, unwrapped (Smart UV Project);
#   3. bake, from the full model onto LOD1, with Cycles on the CPU, a fixed seed and sample count:
#      a tangent-space normal map (OpenGL / glTF axes) and roughness, and, through an emission pass
#      of each material's own inputs, its base colour (a metal's too: Cycles' diffuse colour pass is
#      black for metals), its metallic input and its ORM texture's red channel (occlusion);
#   4. LOD2: LOD1 simplified to a quarter, same UVs, same textures;
#   5. collider hulls from the collision points extract_usd.py wrote: one convex hull per collider
#      (a box for boundingCube, the hull's bounding sphere for boundingSphere).
#
# Everything is written to <build_dir>/bake/ as raw little-endian float32 files and bake.json, in
# Blender's Z-up metres; pack.js turns them into the shipped files. Deterministic by construction:
# fixed seed, samples, margin, unwrap angle and margin, and the thread count build.js passes.

import json
import math
import os
import struct
import sys

import bmesh
import bpy

args = sys.argv[sys.argv.index('--') + 1:]
asset_dir, build_dir, config_path = args[0], args[1], args[2]
with open(config_path, encoding='utf-8') as f:
    cfg = json.load(f)
out_dir = os.path.join(build_dir, 'bake')
os.makedirs(out_dir, exist_ok=True)


def log(msg):
    print('[bake] ' + msg, flush=True)


# ---- 1. import ----
bpy.ops.wm.read_factory_settings(use_empty=True)
usd_dir = os.path.join(asset_dir, 'simready_usd')
root = [f for f in sorted(os.listdir(usd_dir)) if f.startswith('sm_') and f.split('.')[-1] in ('usd', 'usda')][0]
bpy.ops.wm.usd_import(
    filepath=os.path.join(usd_dir, root),
    scale=cfg['meters_per_unit'],
    set_frame_range=False,
    import_cameras=False, import_curves=False, import_lights=False, import_materials=True,
    import_meshes=True, import_volumes=False, import_shapes=False, import_skeletons=False,
    import_blendshapes=False, import_points=False, import_subdivision=False,
    support_scene_instancing=False, import_visible_only=True, import_guide=False,
    import_proxy=False, import_render=True, import_usd_preview=True, mtl_purpose='MTL_PREVIEW',
    import_textures_mode='IMPORT_NONE', create_world_material=False,
)
high = sorted([o for o in bpy.context.scene.objects if o.type == 'MESH'], key=lambda o: o.name)
if not high:
    raise SystemExit('no meshes imported')
for o in bpy.context.scene.objects:
    o.hide_render = False
log('imported %d meshes' % len(high))

# The brand check's repaints (scripts/simready/repaint.js): a texture with a maker's mark painted
# out replaces the original wherever the materials use it.
overrides = cfg.get('texture_overrides') or {}
for img in bpy.data.images:
    base = os.path.basename(bpy.path.abspath(img.filepath)) if img.filepath else ''
    if base in overrides:
        img.filepath = overrides[base]
        img.reload()
        log('repainted texture in use: %s' % base)

# A texture a material names but the download does not hold (the humidifier's grey steel points
# into source_assets/, which is never fetched): its colour output becomes a plain grey, so the bake
# never meets Blender's magenta "missing image" colour. Listed in bake.json for SOURCE.md.
missing = []
for o in high:
    for s in o.material_slots:
        m = s.material
        if not m or not m.use_nodes:
            continue
        nt = m.node_tree
        for n in list(nt.nodes):
            if n.type != 'TEX_IMAGE':
                continue
            # The importer leaves an image node with no image when the file is not there (seen on the
            # humidifier); a node whose image file is gone counts the same.
            if n.image is not None and n.image.filepath and os.path.exists(bpy.path.abspath(n.image.filepath)):
                continue
            name = os.path.basename(bpy.path.abspath(n.image.filepath)) if n.image is not None and n.image.filepath else '(a texture the importer could not find)'
            if name not in missing:
                missing.append(name)
            grey = nt.nodes.new('ShaderNodeRGB')
            grey.outputs[0].default_value = (0.45, 0.45, 0.47, 1.0)
            for l in list(n.outputs['Color'].links):
                nt.links.new(grey.outputs[0], l.to_socket)
            for l in list(n.outputs['Alpha'].links):
                nt.links.remove(l)
            nt.nodes.remove(n)
if missing:
    log('textures not in the download, painted plain grey: %s' % ', '.join(missing))

# ---- 2. LOD1 ----
bpy.ops.object.select_all(action='DESELECT')
for o in high:
    o.select_set(True)
bpy.context.view_layer.objects.active = high[0]
bpy.ops.object.duplicate()
dups = list(bpy.context.selected_objects)
bpy.context.view_layer.objects.active = dups[0]
# Instances and modifiers become real geometry; transforms are applied so LOD1 is in world space.
bpy.ops.object.make_single_user(object=True, obdata=True)
bpy.ops.object.convert(target='MESH')
bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
if len(dups) > 1:
    bpy.ops.object.join()
low = bpy.context.view_layer.objects.active
low.name = 'lod1'
low.data.materials.clear()
for a in list(low.data.attributes):
    pass
while low.data.uv_layers:
    low.data.uv_layers.remove(low.data.uv_layers[0])
if low.data.has_custom_normals:
    bpy.ops.mesh.customdata_custom_splitnormals_clear()

bm = bmesh.new()
bm.from_mesh(low.data)
bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=cfg['weld_m'])
bmesh.ops.triangulate(bm, faces=bm.faces)
bm.to_mesh(low.data)
bm.free()
source_tris = len(low.data.polygons)


def decimate_to(obj, target):
    n = len(obj.data.polygons)
    if n <= target:
        return n
    mod = obj.modifiers.new('decimate', 'DECIMATE')
    mod.decimate_type = 'COLLAPSE'
    mod.ratio = target / n
    mod.use_collapse_triangulate = True
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return len(obj.data.polygons)


lod1_tris = decimate_to(low, cfg['lod1_triangles'])
bpy.context.view_layer.objects.active = low
bpy.ops.object.select_all(action='DESELECT')
low.select_set(True)
bpy.ops.object.shade_smooth_by_angle(angle=math.radians(30.0))
low.data.uv_layers.new(name='bake')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=math.radians(66.0), island_margin=cfg['island_margin'], area_weight=0.0, correct_aspect=True, scale_to_bounds=False)
bpy.ops.object.mode_set(mode='OBJECT')
log('LOD1: %d -> %d triangles' % (source_tris, lod1_tris))

# ---- 3. bake ----
scene = bpy.context.scene
scene.render.engine = 'CYCLES'
scene.cycles.device = 'CPU'
scene.cycles.samples = cfg['samples']
scene.cycles.seed = 0
scene.cycles.use_denoising = False
scene.render.threads_mode = 'FIXED'
scene.render.threads = cfg['threads']
scene.render.bake.margin = cfg['margin_px']
scene.render.bake.margin_type = 'EXTEND'

bake_mat = bpy.data.materials.new('bake_target')
bake_mat.use_nodes = True
low.data.materials.append(bake_mat)
img_node = bake_mat.node_tree.nodes.new('ShaderNodeTexImage')
bake_mat.node_tree.nodes.active = img_node

diag = math.dist(low.bound_box[0], low.bound_box[6]) if low.bound_box else 1.0
extrusion = max(0.001, diag * cfg['cage_fraction'])


def select_for_bake():
    bpy.ops.object.select_all(action='DESELECT')
    for o in high:
        o.select_set(True)
    low.select_set(True)
    bpy.context.view_layer.objects.active = low


def bake_into(name, px, kind, **kw):
    img = bpy.data.images.new(name, px, px, alpha=True, float_buffer=True)
    img.colorspace_settings.name = 'Non-Color'
    img.generated_color = (0.0, 0.0, 0.0, 1.0)
    img_node.image = img
    select_for_bake()
    bpy.ops.object.bake(type=kind, use_selected_to_active=True, cage_extrusion=extrusion, max_ray_distance=extrusion * 4.0,
                        margin=cfg['margin_px'], margin_type='EXTEND', target='IMAGE_TEXTURES', use_clear=True, **kw)
    path = os.path.join(out_dir, name + '.f32')
    buf = [0.0] * (px * px * 4)
    img.pixels.foreach_get(buf)
    with open(path, 'wb') as f:
        f.write(struct.pack('<%df' % len(buf), *buf))
    return {'file': name + '.f32', 'width': px, 'height': px}


def principled(mat):
    if not mat or not mat.use_nodes:
        return None
    for n in mat.node_tree.nodes:
        if n.type == 'BSDF_PRINCIPLED':
            return n
    return None


def output_node(mat):
    for n in mat.node_tree.nodes:
        if n.type == 'OUTPUT_MATERIAL' and n.is_active_output:
            return n
    for n in mat.node_tree.nodes:
        if n.type == 'OUTPUT_MATERIAL':
            return n
    return None


high_mats = []
for o in high:
    for s in o.material_slots:
        if s.material and s.material not in high_mats:
            high_mats.append(s.material)
saved = {}
for m in high_mats:
    out = output_node(m)
    if out and out.inputs['Surface'].links:
        saved[m.name] = out.inputs['Surface'].links[0].from_socket


def emit_pass(source):
    """Rewire every high material so its surface is an emission of `source` (a function of the
    material returning (socket or None, constant)), for an EMIT bake of a single channel."""
    for m in high_mats:
        nt = m.node_tree
        out = output_node(m)
        if not out:
            continue
        em = nt.nodes.get('__bake_emit') or nt.nodes.new('ShaderNodeEmission')
        em.name = '__bake_emit'
        for l in list(em.inputs['Color'].links):
            nt.links.remove(l)
        sock, const = source(m)
        if sock is not None:
            nt.links.new(sock, em.inputs['Color'])
        elif isinstance(const, tuple):
            em.inputs['Color'].default_value = const
        else:
            em.inputs['Color'].default_value = (const, const, const, 1.0)
        em.inputs['Strength'].default_value = 1.0
        nt.links.new(em.outputs['Emission'], out.inputs['Surface'])


def restore_surfaces():
    for m in high_mats:
        out = output_node(m)
        if out and m.name in saved:
            m.node_tree.links.new(saved[m.name], out.inputs['Surface'])


def base_colour_source(m):
    """The material's own base colour (its texture or its constant). Baked through an emission
    pass, not Cycles' DIFFUSE colour pass: that pass is the diffuse share of the surface, which is
    black for a metal, so stainless steel and chrome came out black (seen on the stainless table and
    the tool chest, 2026-11-09)."""
    p = principled(m)
    if not p:
        return None, 0.8
    inp = p.inputs['Base Color']
    if inp.links:
        return inp.links[0].from_socket, 0.0
    c = inp.default_value
    return None, (float(c[0]), float(c[1]), float(c[2]), 1.0)


def metallic_source(m):
    p = principled(m)
    if not p:
        return None, 0.0
    inp = p.inputs['Metallic']
    if inp.links:
        return inp.links[0].from_socket, 0.0
    return None, float(inp.default_value)


def occlusion_source(m):
    """The red channel of the material's ORM texture (NVIDIA's packing), else no occlusion."""
    nt = m.node_tree
    for n in nt.nodes:
        if n.type == 'TEX_IMAGE' and n.image and 'orm' in os.path.basename(n.image.filepath).lower():
            sep = nt.nodes.get('__bake_sep') or nt.nodes.new('ShaderNodeSeparateColor')
            sep.name = '__bake_sep'
            nt.links.new(n.outputs['Color'], sep.inputs['Color'])
            return sep.outputs['Red'], 1.0
    return None, 1.0


albedo_px, pbr_px = cfg['albedo_px'], cfg['pbr_px']
textures = {}
emit_pass(base_colour_source)
textures['albedo'] = bake_into('albedo', albedo_px, 'EMIT')
restore_surfaces()
textures['normal'] = bake_into('normal', pbr_px, 'NORMAL', normal_space='TANGENT', normal_r='POS_X', normal_g='POS_Y', normal_b='POS_Z')
textures['roughness'] = bake_into('roughness', pbr_px, 'ROUGHNESS')
emit_pass(metallic_source)
textures['metallic'] = bake_into('metallic', pbr_px, 'EMIT')
emit_pass(occlusion_source)
textures['occlusion'] = bake_into('occlusion', pbr_px, 'EMIT')
restore_surfaces()
log('baked %d textures' % len(textures))

# ---- 4. LOD2 ----
bpy.ops.object.select_all(action='DESELECT')
low.select_set(True)
bpy.context.view_layer.objects.active = low
bpy.ops.object.duplicate()
lod2 = bpy.context.view_layer.objects.active
lod2.name = 'lod2'
lod2_tris = decimate_to(lod2, max(12, int(lod1_tris * cfg['lod2_ratio'])))
log('LOD2: %d triangles' % lod2_tris)


def write_mesh(obj, name):
    me = obj.data
    me.calc_loop_triangles()
    uv = me.uv_layers['bake'].data
    normals = me.corner_normals
    floats = []
    for tri in me.loop_triangles:
        for li in tri.loops:
            v = me.vertices[me.loops[li].vertex_index].co
            n = normals[li].vector
            t = uv[li].uv
            floats.extend((v.x, v.y, v.z, n.x, n.y, n.z, t.x, t.y))
    with open(os.path.join(out_dir, name + '.f32'), 'wb') as f:
        f.write(struct.pack('<%df' % len(floats), *floats))
    return {'file': name + '.f32', 'corners': len(floats) // 8, 'triangles': len(me.loop_triangles)}


meshes = {'lod1': write_mesh(low, 'lod1'), 'lod2': write_mesh(lod2, 'lod2')}

# ---- 5. collider hulls ----
hulls = []
cpath = os.path.join(build_dir, 'colliders.json')
if os.path.exists(cpath):
    with open(cpath, encoding='utf-8') as f:
        cols = json.load(f)
    with open(os.path.join(build_dir, 'colliders.f32'), 'rb') as f:
        allpts = f.read()
    for i, c in enumerate(cols['colliders']):
        n = c['count']
        pts = struct.unpack_from('<%df' % (n * 3), allpts, c['offset'] * 4)
        bm = bmesh.new()
        grid = cfg['hull_grid_m']
        seen = set()
        for k in range(n):
            q = (round(pts[3 * k] / grid), round(pts[3 * k + 1] / grid), round(pts[3 * k + 2] / grid))
            if q in seen:
                continue
            seen.add(q)
            bm.verts.new((pts[3 * k], pts[3 * k + 1], pts[3 * k + 2]))
        if len(bm.verts) < 4:
            bm.free()
            continue
        res = bmesh.ops.convex_hull(bm, input=bm.verts, use_existing_faces=False)
        for v in [v for v in res['geom_interior'] if isinstance(v, bmesh.types.BMVert)] + [v for v in res['geom_unused'] if isinstance(v, bmesh.types.BMVert)]:
            if v.is_valid:
                bm.verts.remove(v)
        bmesh.ops.triangulate(bm, faces=bm.faces)
        bm.verts.index_update()
        verts = [tuple(v.co) for v in bm.verts]
        tris = [[v.index for v in f.verts] for f in bm.faces]
        bm.free()
        hulls.append({'collider': c['path'], 'approximation': c['approximation'], 'body': c['body'], 'vertices': verts, 'triangles': tris})
log('%d collider hulls' % len(hulls))

with open(os.path.join(out_dir, 'bake.json'), 'w', encoding='utf-8', newline='\n') as f:
    json.dump({
        'source_triangles': source_tris,
        'meshes': meshes,
        'textures': textures,
        'hulls': hulls,
        'missing_textures': sorted(missing),
        'blender': bpy.app.version_string,
        'config': cfg,
    }, f, indent=1)
    f.write('\n')
