# AGENTS.md — Engine Development Rules & Best Practices

## 1. Context & Quota Management (Strict)
* **Be Succinct:** Do not rewrite unchanged files. When updating code, modify only the specific functions or lines required.
* **No Monoliths:** Keep files modular. If a file exceeds 300 lines of code, split it into a distinct submodule (e.g., separate `renderer/mod.rs`, `renderer/pipeline.rs`, `renderer/primitives.rs`).
* **Compile First:** Always run `cargo check` or `cargo build` immediately after a code change to verify syntax before proceeding to the next step.

## 2. Architectural Restrictions
* **No Reinventing the Wheel:** Do not implement custom 3D math, matrix operations, or quaternion rotations. Use the `glam` crate exclusively.
* **Component-Driven Separation:** Keep the Editor UI (`egui`), the Rendering Pipeline (`wgpu`), and the Lua State Machine (`mlua`) decoupled. They must communicate via messages or clear structural boundaries, never via messy global variables.
* **Mesh Reusability:** Ensure the four primitive meshes (Box, Sphere, Plane, Cylinder) are generated once into reusable Vertex/Index buffers rather than duplicated across entities.

## 3. Workflow for Feature Additions
1. Read the existing codebase to map out dependencies.
2. Formulate the minimum required code change.
3. Apply the fix/feature.
4. Run `cargo check`. If it fails, fix the compiler error directly based on the terminal output. Do not rewrite the entire file to solve a single compile error.

I have a file called `auxmd.md` which is gitignored, where i put throaway notes which are useful in the short-term

