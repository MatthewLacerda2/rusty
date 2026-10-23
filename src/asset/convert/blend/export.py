# The one place the `.blend` export settings live (#1072). `blend::run` runs it as
#   blender --background --factory-startup --disable-autoexec <source>
#           --python-exit-code 1 --python-expr <this file> -- <export dir>
# and `blend::pack` turns what it writes into the cached `.glb`.
#
# Only the images a material uses are looked at. Each one the conversion makes (a
# packed one, a generated one) is written into `<export dir>/made/` first, so the
# exporter's keep-originals option references it there, and is given its identity:
# the image's name, the first image of a name keeping it and a later one (a linked
# library's namesake) taking `.1`, `.2`, ... An image that is a file on disk is
# referenced where it lies. What the conversion looked at, made and left out is
# written to `<export dir>/rusty.json`.
import bpy
import json
import os
import sys

out = sys.argv[sys.argv.index("--") + 1]
made = os.path.join(out, "made")
os.makedirs(made, exist_ok=True)
report = {"libraries": [], "images": [], "made": [], "skipped": []}


def kind(data):
    """The format packed bytes are in, by their magic number: the name may lie."""
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:3] == b"\xff\xd8\xff":
        return "jpg"
    return None


def used_images():
    """The images the materials of the file's objects sample, in file order."""
    used, trees, seen = set(), [], set()
    for obj in bpy.data.objects:
        for slot in obj.material_slots:
            if slot.material is not None and slot.material.node_tree is not None:
                trees.append(slot.material.node_tree)
    while trees:
        tree = trees.pop()
        if tree in seen:
            continue
        seen.add(tree)
        for node in tree.nodes:
            if node.type == "TEX_IMAGE" and node.image is not None:
                used.add(node.image)
            elif node.type == "GROUP" and node.node_tree is not None:
                trees.append(node.node_tree)
    return [image for image in bpy.data.images if image in used]


def identity(name, taken):
    unique, n = name, 0
    while unique in taken:
        n += 1
        unique = "%s.%d" % (name, n)
    taken.add(unique)
    return unique


for library in bpy.data.libraries:
    report["libraries"].append(os.path.normpath(bpy.path.abspath(library.filepath)))

taken = set()
for n, image in enumerate(used_images()):
    if image.type not in ("IMAGE", "UV_TEST"):
        continue
    if image.packed_file is None and image.source == "FILE":
        path = bpy.path.abspath(image.filepath, library=image.library)
        report["images"].append({"name": image.name, "path": os.path.normpath(path)})
        continue
    ext = os.path.splitext(image.filepath)[1].lstrip(".").lower()
    try:
        data = image.packed_file.data if image.packed_file is not None else b""
        raw = kind(data) or ("tga" if data and ext == "tga" else None)
        if raw is not None:
            path = os.path.join(made, "%d.%s" % (n, raw))
            with open(path, "wb") as f:
                f.write(data)
        else:
            path = os.path.join(made, "%d.png" % n)
            image.filepath_raw = path
            image.file_format = "PNG"
            image.save()
        image.filepath = path
        name = identity(image.name, taken)
        report["made"].append({"file": os.path.basename(path), "name": name})
    except Exception as e:
        report["skipped"].append({"what": "Blender image `%s`" % image.name, "why": str(e)})

bpy.ops.export_scene.gltf(
    filepath=os.path.join(out, "model.gltf"),
    export_format="GLTF_SEPARATE",
    export_keep_originals=True,
    export_yup=True,
    export_apply=True,
    export_animations=True,
    export_skins=True,
    export_materials="EXPORT",
)

with open(os.path.join(out, "rusty.json"), "w") as f:
    json.dump(report, f)
