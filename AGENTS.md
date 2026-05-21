1. Target Directory & Module Structure (Strict)
The engine architecture must adhere to this isolated modular layout. Do not introduce flat files to the root src/ directory unless modifying global coordination in main.rs.

.
├── Cargo.toml
├── AGENTS.md
├── auxmd.md                 # Gitignored: Short-term operator notes and scratchpad
├── assets/
│   ├── shaders/
│   │   └── shader.wgsl      # All WGSL shader source code lives here
│   ├── scripts/
│   │   └── bot.lua          # AI and gameplay logic scripts
│   └── models/              # External FBX assets
└── src/
    ├── main.rs              # App entry point, winit loop, subsystem orchestration
    ├── render/              # RENDER MODULE
    │   ├── mod.rs           # wgpu graphics context, rendering initialization
    │   ├── pipeline.rs      # Render pipelines, bind groups, shader compilation
    │   └── mesh.rs          # Primitive vertex buffers & FBX model loading
    ├── core/                # ENGINE CORE MODULE
    │   ├── scene.rs         # Entity-Component structures, scene graph state
    │   └── input.rs         # Keyboard engine (WASD, Arrow Keys look control)
    ├── physics/             # PHYSICS & COLLISION MODULE
    │   ├── mod.rs           # Simple constant gravity & box-clipping boundaries
    │   └── raycast.rs       # Hitscan shooting mathematics (Ray-AABB testing)
    ├── scripting/           # SCRIPTING MODULE
    │   ├── mod.rs           # mlua state runtime management
    │   └── bindings.rs      # Safe Rust-to-Lua API bindings (Transform, Animator)
    └── navigation/          # NAVIGATION MODULE
        └── mod.rs           # Pathfinder waypoint graphs and A* implementation
2. Context & Quota Management (Strict)
Be Succinct: Do not rewrite unchanged files. When updating code, modify or output only the specific functions or lines required.

No Monoliths: Keep files modular. If a module file exceeds 300 lines of code, split it into targeted sub-files within its module directory.

Compile First: Always run cargo check or cargo build immediately after a code change to verify syntax before proceeding to subsequent tasks.

Scratchpad Awareness: Always read auxmd.md at the start of a session if indicated by the operator to parse raw error dumps or short-term execution logs.

3. Architectural Restrictions
No Reinventing the Wheel: Do not implement custom 3D math, matrix operations, or quaternion rotations. Use the glam crate exclusively.

Component-Driven Separation: Keep the Editor UI (egui), the Rendering Pipeline (wgpu), and the Lua State Machine (mlua) decoupled. They must communicate via clean structural parameters or messages, never via global mutable variables.

Mesh Reusability: Ensure the four primitive meshes (Box, Sphere, Plane, Cylinder) are generated once into reusable Vertex/Index buffers rather than duplicated across individual entities.

4. Workflow for Feature Additions
Read the existing codebase and AGENTS.md layout to map out boundaries.

Formulate the minimum required code change.

Apply the fix/feature within the designated module directory.

Run cargo check. If it fails, fix the compiler error directly based on the terminal output. Do not rewrite a whole file to solve a single compile error.