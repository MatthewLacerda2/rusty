# Makes packed_formats.blend (#1072), the fixture for packed images that are not
# PNG or JPEG: the cube "Bmp" samples a packed BMP ("Bitmap", blue) and the cube
# "Float" a packed float OpenEXR ("Radiance", linear 0.5 grey). Both must import
# as decodable 8-bit PNGs.
# Run from this folder:
#   blender --background --factory-startup --python make_packed_formats_blend.py
import bpy
import os
import tempfile

here = os.path.dirname(os.path.abspath(__file__))
scratch = tempfile.mkdtemp()
bpy.ops.wm.read_factory_settings(use_empty=True)


def packed(name, file_name, file_format, rgba, float_buffer):
    made = bpy.data.images.new(name + ".made", 2, 2, float_buffer=float_buffer)
    made.pixels = list(rgba) * 4
    path = os.path.join(scratch, file_name)
    made.filepath_raw = path
    made.file_format = file_format
    made.save()
    bpy.data.images.remove(made)
    image = bpy.data.images.load(path)
    image.name = name
    image.pack()
    return image


def cube(name, location, image):
    bpy.ops.mesh.primitive_cube_add(location=location)
    obj = bpy.context.active_object
    obj.name = name
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    tex = m.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = image
    bsdf = m.node_tree.nodes["Principled BSDF"]
    m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    obj.data.materials.append(m)


cube("Bmp", (0, 0, 0), packed("Bitmap", "bitmap.bmp", "BMP", (0, 0, 1, 1), False))
cube("Float", (3, 0, 0), packed("Radiance", "radiance.exr", "OPEN_EXR", (0.5, 0.5, 0.5, 1), True))

# No `.blend1` backup beside a fixture saved over.
bpy.context.preferences.filepaths.save_version = 0
bpy.ops.wm.save_as_mainfile(
    filepath=os.path.join(here, "packed_formats.blend"), relative_remap=True, compress=True
)
