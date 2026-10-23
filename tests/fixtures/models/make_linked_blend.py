# Makes linked.blend and linked_lib.blend (#1072), the fixture for an image's
# identity: linked.blend's cube samples a packed red image named "Wood", and the
# crate it links from linked_lib.blend samples that file's packed blue "Wood".
# Run from this folder:
#   blender --background --factory-startup --python make_linked_blend.py
import bpy
import os

here = os.path.dirname(os.path.abspath(__file__))


def textured(obj_name, material_name, rgba, location):
    bpy.ops.mesh.primitive_cube_add(location=location)
    obj = bpy.context.active_object
    obj.name = obj_name
    image = bpy.data.images.new("Wood", 2, 2)
    image.pixels = list(rgba) * 4
    image.filepath_raw = "//Wood.png"
    image.file_format = "PNG"
    image.pack()
    m = bpy.data.materials.new(material_name)
    m.use_nodes = True
    tex = m.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    bsdf = m.node_tree.nodes["Principled BSDF"]
    m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    obj.data.materials.append(m)


def save(name):
    path = os.path.join(here, name)
    # No `.blend1` backup beside a fixture saved over.
    bpy.context.preferences.filepaths.save_version = 0
    bpy.ops.wm.save_as_mainfile(filepath=path, relative_remap=True, compress=True)
    return path


bpy.ops.wm.read_factory_settings(use_empty=True)
textured("Crate", "Linked", (0.0, 0.0, 1.0, 1.0), (3, 0, 0))
lib = save("linked_lib.blend")

bpy.ops.wm.read_factory_settings(use_empty=True)
textured("Cube", "Local", (1.0, 0.0, 0.0, 1.0), (0, 0, 0))
with bpy.data.libraries.load(lib, link=True, relative=True) as (src, dst):
    dst.objects = ["Crate"]
for obj in dst.objects:
    bpy.context.scene.collection.objects.link(obj)
save("linked.blend")
