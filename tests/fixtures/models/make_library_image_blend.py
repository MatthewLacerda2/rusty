# Makes library_image/ (#1072), the fixture for a linked library's image on disk:
# direct.blend's cube samples a packed red image named "Wood", and the crate it
# links from sub2/lib.blend samples sub2/tex.png, a green file on disk also named
# "Wood". The glTF exporter resolves a library's relative image path against the
# main file's folder; the conversion must find sub2/tex.png, decoy or not.
# Run from this folder:
#   blender --background --factory-startup --python make_library_image_blend.py
import bpy
import os

here = os.path.join(os.path.dirname(os.path.abspath(__file__)), "library_image")
sub = os.path.join(here, "sub2")
os.makedirs(sub, exist_ok=True)


def material(name, image):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    tex = m.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    bsdf = m.node_tree.nodes["Principled BSDF"]
    m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    return m


def cube(obj_name, location, m):
    bpy.ops.mesh.primitive_cube_add(location=location)
    obj = bpy.context.active_object
    obj.name = obj_name
    obj.data.materials.append(m)


def save(path):
    # No `.blend1` backup beside a fixture saved over.
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=path, relative_remap=True, compress=True)


bpy.ops.wm.read_factory_settings(use_empty=True)
green = bpy.data.images.new("Wood", 2, 2)
green.pixels = [0.0, 1.0, 0.0, 1.0] * 4
green.filepath_raw = os.path.join(sub, "tex.png")
green.file_format = "PNG"
green.save()
green.filepath = os.path.join(sub, "tex.png")
cube("Crate", (3, 0, 0), material("Linked", green))
lib = os.path.join(sub, "lib.blend")
save(lib)

bpy.ops.wm.read_factory_settings(use_empty=True)
red = bpy.data.images.new("Wood", 2, 2)
red.pixels = [1.0, 0.0, 0.0, 1.0] * 4
red.filepath_raw = "//Wood.png"
red.file_format = "PNG"
red.pack()
cube("Cube", (0, 0, 0), material("Local", red))
with bpy.data.libraries.load(lib, link=True, relative=True) as (src, dst):
    dst.objects = ["Crate"]
for obj in dst.objects:
    bpy.context.scene.collection.objects.link(obj)
save(os.path.join(here, "direct.blend"))
