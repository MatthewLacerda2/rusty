"""The merge and mutation helpers' tests, one subfolder per script or package.

`make scripts` runs them with `.github/scripts` as the top-level directory, so
`merging` and this package import by name. A script with a hyphen in its name
is not importable; [`load`] reads it by path.
"""

import importlib.util
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent


def load(script: str, name: str | None = None):
    """The module `.github/scripts/<script>`, loaded by path."""
    spec = importlib.util.spec_from_file_location(name or Path(script).stem.replace("-", "_"), SCRIPTS / script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
