# The one place the `.blend` export settings live (#1072). `blend::run` runs it as
#   blender --background --factory-startup --disable-autoexec <source>
#           --python-exit-code 1 --python-expr <this file> -- <export dir>
# and `blend::pack` turns what it writes into the cached `.glb`.
#
# Every image the conversion makes (a packed one, a generated one) is written into
# `<export dir>/made/` first, so the exporter's keep-originals option references
# it there; an image that is a file on disk is referenced where it lies. What the
# conversion looked at and left out is written to `<export dir>/rusty.json`.
import bpy
import json
import os
import sys

out = sys.argv[sys.argv.index("--") + 1]
made = os.path.join(out, "made")
os.makedirs(made, exist_ok=True)
READABLE = {"png", "jpg", "jpeg", "tga"}
report = {"libraries": [], "images": [], "skipped": []}

for library in bpy.data.libraries:
    report["libraries"].append(os.path.normpath(bpy.path.abspath(library.filepath)))

for n, image in enumerate(bpy.data.images):
    if image.type not in ("IMAGE", "UV_TEST"):
        continue
    if image.packed_file is None and image.source == "FILE":
        path = bpy.path.abspath(image.filepath, library=image.library)
        report["images"].append({"name": image.name, "path": os.path.normpath(path)})
        continue
    # Numbered, so two images of one name never share a file.
    stem = "%d" % n
    ext = os.path.splitext(image.filepath)[1].lstrip(".").lower()
    try:
        if image.packed_file is not None and ext in READABLE:
            path = os.path.join(made, "%s.%s" % (stem, ext))
            with open(path, "wb") as f:
                f.write(image.packed_file.data)
        else:
            path = os.path.join(made, stem + ".png")
            image.filepath_raw = path
            image.file_format = "PNG"
            image.save()
        image.filepath = path
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
