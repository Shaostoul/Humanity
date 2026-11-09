# extract_usd.py - read one NVIDIA SimReady asset's USD stage with Pixar's own `pxr` bindings and
# write what the rest of the pipeline needs as plain JSON (docs/design/simready-props.md, section 4,
# step 2). Runs INSIDE Blender (which bundles pxr), using pxr only, never bpy:
#
#   blender --background --factory-startup --python-exit-code 1 \
#       --python scripts/simready/extract_usd.py -- <asset_dir> <out_json> [--plan]
#
# Python is used here only because Blender's bundled interpreter is the one place on this machine
# that has pxr without installing anything; everything else in the pipeline is node.
#
# What it writes:
#   * the stage's units (metersPerUnit, kilogramsPerUnit, upAxis), checked;
#   * every layer and asset the root depends on, refusing any that leaves the asset's folder, is a
#     URL, or is absolute somewhere else (the files are untrusted data);
#   * `textures`: the texture files the materials actually reference (resolved or not yet
#     downloaded), as paths relative to the asset folder; with --plan that is ALL it writes, as
#     plan.json, which fetch.js pass 2 reads;
#   * the render meshes' count, triangle count and bounds, in metres, Y up;
#   * the NEUTRAL physics (UsdPhysics only; the PhysX, Newton and MuJoCo variants are forced to
#     Disabled on the session layer, so their payloads are never composed): rigid bodies, mass,
#     density, centre of mass, inertia, colliders and their approximation, physics materials and
#     joints, with positions converted to metres, Y up.
#
# Output is deterministic: prims in stage order, numbers as Python writes floats, keys in a fixed
# order. Nothing here writes outside <out_json>.

import json
import os
import sys

from pxr import Gf, Sdf, Usd, UsdGeom, UsdPhysics, UsdShade, UsdUtils


def fail(msg):
    sys.stderr.write('extract_usd: ' + msg + '\n')
    sys.exit(1)


args = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
if len(args) < 2:
    fail('usage: extract_usd.py -- <asset_dir> <out_json> [--plan]')
asset_dir = os.path.realpath(args[0])
out_path = args[1]
plan_only = '--plan' in args[2:]

usd_dir = os.path.join(asset_dir, 'simready_usd')
roots = sorted(f for f in os.listdir(usd_dir) if f.startswith('sm_') and f.split('.')[-1] in ('usd', 'usda'))
if len(roots) != 1:
    fail('expected one sm_*.usd root in %s, found %r' % (usd_dir, roots))
root_path = os.path.join(usd_dir, roots[0])


def inside(p):
    """Whether a path (resolved or merely computed) stays inside the asset folder."""
    if '://' in p:
        return False
    rp = os.path.realpath(p)
    return rp == asset_dir or rp.startswith(asset_dir + os.sep)


def rel(p):
    return os.path.relpath(os.path.realpath(p), asset_dir).replace(os.sep, '/')


# ---- dependencies: every layer and asset, and nothing outside the folder ----
layers, assets, unresolved = UsdUtils.ComputeAllDependencies(root_path)
for lyr in layers:
    if not inside(lyr.realPath):
        fail('layer outside the asset folder: %s' % lyr.realPath)
for a in list(assets) + list(unresolved):
    if not inside(a):
        fail('asset path outside the asset folder: %s' % a)

IGNORED = ('/payloads/PhysX/enabled.usda', '/payloads/Newton/enabled.usda', '/payloads/MuJoCo/enabled.usda')
texture_refs = []
outside_refs = []
missing_layers = []
for a in list(assets) + list(unresolved):
    r = rel(a)
    low = r.lower()
    if low.endswith('.mdl'):
        continue  # the MDL shaders are never fetched or read (the legal finding, F4)
    if '/textures/' in low:
        texture_refs.append(r)
    elif low.endswith(('.png', '.jpg', '.jpeg', '.exr', '.tga', '.tif', '.tiff')):
        # A picture outside Textures/ (the humidifier's grey steel points into source_assets/):
        # never fetched, so the bake paints that surface plain grey and SOURCE.md says so.
        outside_refs.append(r)
    elif low.endswith(('.usd', '.usda', '.usdc')):
        if a in unresolved and not any(r.endswith(x) for x in IGNORED) and not r.startswith('simready_usd/runnables/'):
            missing_layers.append(r)
texture_refs = sorted(set(texture_refs))
if missing_layers:
    fail('layers the default composition needs are missing: %r' % missing_layers)

if plan_only:
    with open(out_path, 'w', encoding='utf-8', newline='\n') as f:
        json.dump({'textures': texture_refs, 'outside': sorted(set(outside_refs))}, f, indent=1)
        f.write('\n')
    sys.exit(0)

# ---- the stage, with every solver variant Disabled (session layer, nothing on disk changes) ----
stage = Usd.Stage.Open(root_path)
dp = stage.GetDefaultPrim()
if not dp:
    fail('no default prim')
stage.SetEditTarget(Usd.EditTarget(stage.GetSessionLayer()))
for name in dp.GetVariantSets().GetNames():
    vs = dp.GetVariantSets().GetVariantSet(name)
    if 'Disabled' in vs.GetVariantNames():
        vs.SetVariantSelection('Disabled')

mpu = UsdGeom.GetStageMetersPerUnit(stage)
kpu = UsdPhysics.GetStageKilogramsPerUnit(stage)
up = UsdGeom.GetStageUpAxis(stage)
if up not in ('Y', 'Z'):
    fail('unknown up axis %r' % up)


def to_y_up(v):
    """A stage-space point or vector to metres, Y up (glTF): Z-up (x, y, z) becomes (x, z, -y)."""
    x, y, z = float(v[0]) * mpu, float(v[1]) * mpu, float(v[2]) * mpu
    return [x, z, -y] if up == 'Z' else [x, y, z]


def dir_y_up(v):
    x, y, z = float(v[0]), float(v[1]), float(v[2])
    return [x, z, -y] if up == 'Z' else [x, y, z]


xcache = UsdGeom.XformCache(Usd.TimeCode.Default())
bbox_cache = UsdGeom.BBoxCache(Usd.TimeCode.Default(), [UsdGeom.Tokens.default_, UsdGeom.Tokens.render])

render_meshes = []
tris = 0
lo = [float('inf')] * 3
hi = [float('-inf')] * 3
for prim in Usd.PrimRange(stage.GetPseudoRoot(), Usd.TraverseInstanceProxies()):
    if prim.GetTypeName() != 'Mesh':
        continue
    img = UsdGeom.Imageable(prim)
    if img.ComputePurpose() not in (UsdGeom.Tokens.default_, UsdGeom.Tokens.render):
        continue
    if img.ComputeVisibility() == UsdGeom.Tokens.invisible:
        continue
    mesh = UsdGeom.Mesh(prim)
    fvc = mesh.GetFaceVertexCountsAttr().Get() or []
    t = sum(max(0, c - 2) for c in fvc)
    tris += t
    pts = mesh.GetPointsAttr().Get() or []
    m = xcache.GetLocalToWorldTransform(prim)
    for p in pts:
        w = to_y_up(m.Transform(Gf.Vec3d(p)))
        for k in range(3):
            lo[k] = min(lo[k], w[k])
            hi[k] = max(hi[k], w[k])
    mat, _ = UsdShade.MaterialBindingAPI(prim).ComputeBoundMaterial()
    render_meshes.append({'path': str(prim.GetPath()), 'triangles': t, 'material': str(mat.GetPath()) if mat else None})

if not render_meshes:
    fail('no render meshes')


def attr(prim, name):
    a = prim.GetAttribute(name)
    if not a or not a.HasAuthoredValue():
        return None
    v = a.Get()
    return v


def num(v, scale=1.0):
    """A number for the JSON, or None when absent or not finite (an unlimited joint is +-inf)."""
    if v is None:
        return None
    x = float(v) * scale
    return x if x == x and abs(x) != float('inf') else None


# ---- physics: rigid bodies, mass, colliders, materials, joints (UsdPhysics only) ----
bodies = []
colliders = []
joints = []
phys_materials = {}


def physics_material(prim):
    mat, _ = UsdShade.MaterialBindingAPI(prim).ComputeBoundMaterial('physics')
    if not mat:
        return None
    key = str(mat.GetPath())
    if key not in phys_materials:
        mp = mat.GetPrim()
        api = UsdPhysics.MaterialAPI(mp)
        phys_materials[key] = {
            'static_friction': num(api.GetStaticFrictionAttr().Get()) if mp.HasAPI(UsdPhysics.MaterialAPI) else None,
            'dynamic_friction': num(api.GetDynamicFrictionAttr().Get()) if mp.HasAPI(UsdPhysics.MaterialAPI) else None,
            'restitution': num(api.GetRestitutionAttr().Get()) if mp.HasAPI(UsdPhysics.MaterialAPI) else None,
            'density_kg_m3': num(attr(mp, 'physics:density'), kpu / (mpu ** 3)) if attr(mp, 'physics:density') else None,
        }
    return key


def mass_of(prim):
    if not prim.HasAPI(UsdPhysics.MassAPI):
        return None
    out = {}
    m = attr(prim, 'physics:mass')
    if m is not None and m == m and 0 < float(m) < 1e30:
        out['mass_kg'] = float(m) * kpu
    d = attr(prim, 'physics:density')
    if d is not None and d == d and 0 < float(d) < 1e30:
        out['density_kg_m3'] = float(d) * kpu / (mpu ** 3)
    com = attr(prim, 'physics:centerOfMass')
    # USD's "not given" for a centre of mass is -inf; some assets carry NaN. Either way: no value.
    if com is not None and all(c == c and abs(c) < 1e30 for c in com):
        # Centre of mass is in the prim's local space: carry it to the world, then Y up.
        w = xcache.GetLocalToWorldTransform(prim).Transform(Gf.Vec3d(com))
        out['center_of_mass_m'] = to_y_up(w)
    di = attr(prim, 'physics:diagonalInertia')
    if di is not None and all(x == x and abs(x) < 1e30 for x in di) and any(float(x) > 0 for x in di):
        s = kpu * mpu * mpu
        out['diagonal_inertia_kg_m2'] = [float(di[0]) * s, float(di[1]) * s, float(di[2]) * s]
    pa = attr(prim, 'physics:principalAxes')
    if pa is not None and all(c == c for c in (pa.GetReal(),) + tuple(pa.GetImaginary())):
        out['principal_axes_quat_wxyz'] = [float(pa.GetReal()), float(pa.GetImaginary()[0]), float(pa.GetImaginary()[1]), float(pa.GetImaginary()[2])]
    return out or None


for prim in Usd.PrimRange(stage.GetPseudoRoot(), Usd.TraverseInstanceProxies()):
    path = str(prim.GetPath())
    if prim.HasAPI(UsdPhysics.RigidBodyAPI):
        rb = UsdPhysics.RigidBodyAPI(prim)
        bodies.append({
            'path': path,
            'enabled': bool(rb.GetRigidBodyEnabledAttr().Get()),
            'kinematic': bool(rb.GetKinematicEnabledAttr().Get()),
            'mass': mass_of(prim),
        })
    if prim.HasAPI(UsdPhysics.CollisionAPI):
        approx = None
        if prim.HasAPI(UsdPhysics.MeshCollisionAPI):
            approx = str(UsdPhysics.MeshCollisionAPI(prim).GetApproximationAttr().Get())
        elif prim.GetTypeName() == 'Mesh':
            approx = 'none'
        else:
            approx = prim.GetTypeName().lower()  # an implicit shape: cube, sphere, capsule, cylinder
        body = None
        p = prim
        while p and p.GetPath() != Sdf.Path.absoluteRootPath:
            if p.HasAPI(UsdPhysics.RigidBodyAPI):
                body = str(p.GetPath())
                break
            p = p.GetParent()
        colliders.append({
            'path': path,
            'type': prim.GetTypeName(),
            'approximation': approx,
            'body': body,
            'enabled': bool(UsdPhysics.CollisionAPI(prim).GetCollisionEnabledAttr().Get()),
            'mass': mass_of(prim),
            'physics_material': physics_material(prim),
        })
    if prim.IsA(UsdPhysics.Joint):
        j = UsdPhysics.Joint(prim)
        b0 = j.GetBody0Rel().GetTargets()
        b1 = j.GetBody1Rel().GetTargets()
        rec = {
            'path': path,
            'type': prim.GetTypeName(),
            'body0': str(b0[0]) if b0 else None,
            'body1': str(b1[0]) if b1 else None,
            'local_pos0_m': to_y_up(j.GetLocalPos0Attr().Get() or (0, 0, 0)),
            'local_pos1_m': to_y_up(j.GetLocalPos1Attr().Get() or (0, 0, 0)),
            'enabled': bool(j.GetJointEnabledAttr().Get()),
        }
        if prim.IsA(UsdPhysics.RevoluteJoint):
            r = UsdPhysics.RevoluteJoint(prim)
            rec['axis'] = str(r.GetAxisAttr().Get())
            rec['lower_deg'] = num(r.GetLowerLimitAttr().Get())
            rec['upper_deg'] = num(r.GetUpperLimitAttr().Get())
        elif prim.IsA(UsdPhysics.PrismaticJoint):
            r = UsdPhysics.PrismaticJoint(prim)
            rec['axis'] = str(r.GetAxisAttr().Get())
            rec['lower_m'] = num(r.GetLowerLimitAttr().Get(), mpu)
            rec['upper_m'] = num(r.GetUpperLimitAttr().Get(), mpu)
        joints.append(rec)

# ---- the collision points, for bake_lods.py's hulls (metres, Y up, little-endian float32) ----
import struct  # noqa: E402

col_index = []
col_floats = []
for c in colliders:
    prim = stage.GetPrimAtPath(c['path'])
    m = xcache.GetLocalToWorldTransform(prim)
    pts = []
    if prim.GetTypeName() == 'Mesh':
        pts = [m.Transform(Gf.Vec3d(p)) for p in (UsdGeom.Mesh(prim).GetPointsAttr().Get() or [])]
    else:
        # An implicit shape: the corners of its own bounding box, carried to the world.
        box = UsdGeom.Boundable(prim).ComputeLocalBound(Usd.TimeCode.Default(), UsdGeom.Tokens.default_).GetRange()
        if not box.IsEmpty():
            lo3, hi3 = box.GetMin(), box.GetMax()
            for i in range(8):
                corner = Gf.Vec3d(hi3[0] if i & 1 else lo3[0], hi3[1] if i & 2 else lo3[1], hi3[2] if i & 4 else lo3[2])
                pts.append(m.Transform(corner))
    off = len(col_floats) // 3
    for p in pts:
        col_floats.extend(to_y_up(p))
    col_index.append({'path': c['path'], 'approximation': c['approximation'], 'body': c['body'], 'offset': off * 3, 'count': len(pts)})
out_dir_x = os.path.dirname(os.path.abspath(out_path))
with open(os.path.join(out_dir_x, 'colliders.f32'), 'wb') as f:
    f.write(struct.pack('<%df' % len(col_floats), *col_floats))
with open(os.path.join(out_dir_x, 'colliders.json'), 'w', encoding='utf-8', newline='\n') as f:
    json.dump({'space': 'y_up_m', 'colliders': col_index}, f, indent=1)
    f.write('\n')

with open(os.path.join(asset_dir, 'com.nvidia.simready.packaging.json'), encoding='utf-8') as f:
    packaging = json.load(f)

out = {
    'folder': os.path.basename(asset_dir),
    'package_id': packaging.get('package_id'),
    'license': packaging.get('license'),
    'root': rel(root_path),
    'meters_per_unit': mpu,
    'kilograms_per_unit': kpu,
    'up_axis': up,
    'layers': [rel(l.realPath) for l in layers],
    'textures': texture_refs,
    'textures_outside': sorted(set(outside_refs)),
    'render_meshes': render_meshes,
    'triangles': tris,
    'bounds_min_m': lo,
    'bounds_max_m': hi,
    'physics': {
        'bodies': bodies,
        'colliders': colliders,
        'materials': phys_materials,
        'joints': joints,
    },
    'usd_version': '.'.join(str(x) for x in Usd.GetVersion()),
}
if packaging.get('license') != 'CC-BY-4.0':
    fail('the packaging licence is %r, not CC-BY-4.0' % packaging.get('license'))
with open(out_path, 'w', encoding='utf-8', newline='\n') as f:
    json.dump(out, f, indent=1, allow_nan=False)
    f.write('\n')
