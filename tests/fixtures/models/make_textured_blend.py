# Makes textured.blend (#1072), the `.blend` fixture: a cube whose material samples
# an image packed into the file ("Checker"), and a plane whose material samples
# textured_bricks.png beside it ("Bricks", an image on disk). Run from this folder:
#   blender --background --factory-startup --python make_textured_blend.py
import bpy
import os

here = os.path.dirname(os.path.abspath(__file__))
bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, image):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    tex = m.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    return m


bpy.ops.mesh.primitive_cube_add()
cube = bpy.context.active_object
cube.name = "Cube"
checker = bpy.data.images.new("Checker", 4, 4)
checker.pixels = [1.0, 0.0, 0.0, 1.0] * 16
checker.filepath_raw = "//Checker.png"
checker.file_format = "PNG"
checker.pack()
cube.data.materials.append(material("Packed", checker))

bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
plane = bpy.context.active_object
plane.name = "Plane"
bricks = bpy.data.images.load(os.path.join(here, "textured_bricks.png"))
bricks.name = "Bricks"
plane.data.materials.append(material("External", bricks))

bpy.ops.wm.save_as_mainfile(
    filepath=os.path.join(here, "textured.blend"), relative_remap=True, compress=True
)
