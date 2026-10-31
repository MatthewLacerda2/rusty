//! The classes of scene state and what each lifecycle owes them (#1649).

/// What a piece of scene state is, which decides what every lifecycle does to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// What the user makes: saved, put back on Stop, undoable (fog, sky, layers,
    /// the material library, probe placement, …).
    Authored,
    /// What a bake writes: saved (in the scene or its sidecar), put back on Stop,
    /// never undone (Unity's Undo never reverts a lighting bake).
    BakeOutput,
    /// The World's entities: saved, put back on Stop, and undone through the
    /// World's own journal (`ecs::journal`, #1631), not the scene level.
    Entities,
    /// Which entity the scene names as selected: saved and put back on Stop, but
    /// selecting is no edit, so Undo leaves it.
    Selection,
    /// Play's own requests and marks (queued destroys and loads, `DontDestroyOnLoad`,
    /// the play state, runtime shader overrides): never saved, cleared by Stop.
    Runtime,
    /// Runtime state edit mode keeps across Play (the decals stamped in edit mode,
    /// #1518): never saved, put back on Stop, dropped by a load.
    EditRuntime,
    /// What the host writes every frame (the shader clocks, the metered exposure,
    /// whether a bake runs): never saved, and no lifecycle owes it anything but
    /// leaving it to the next frame's write.
    Frame,
    /// The live scene's identity (#355): never saved, a load mints a new one, and
    /// Stop gives back the one Play started in (#1374).
    Identity,
    /// The text the engine last read or wrote at the scene's file (#1340): never
    /// saved, put back on Stop, replaced by a load.
    DiskRecord,
    /// How far the meshes follow the write log (#1242): never saved; every
    /// document applied re-stamps it, so Stop and a load owe it nothing.
    Follow,
    /// Edit mode's Undo history: never saved, cleared by Play and by a load.
    History,
}

/// A lifecycle that copies, puts back or clears scene state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// `save_to_file`, then `load_from_file` into a fresh scene.
    Save,
    /// Play → Stop: `SceneSnapshot::capture` on Play, `restore` on Stop (and a
    /// demo closing over the edit scene, whose `Origin` is the same snapshot).
    Stop,
    /// An edit-mode step, undone.
    Undo,
    /// `load_from_file` over a live scene in edit mode: File ▸ Open, an edit-mode
    /// `Scene.Load` and the editor's follow of a file changed on disk.
    Load,
    /// A Play-mode `Scene.Load`: the tick's scene-load phase swaps another
    /// document in (`Scene::swap_in`), carrying only the `DontDestroyOnLoad`
    /// survivors. It is judged with the marked entity left behind, so whatever is
    /// keyed by that entity must not cross with its stale id (`promise`).
    PlayLoad,
}

impl Lifecycle {
    pub const ALL: [Lifecycle; 5] = [
        Self::Save,
        Self::Stop,
        Self::Undo,
        Self::Load,
        Self::PlayLoad,
    ];
}

/// What a lifecycle does to a value, as a promise and as an observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The value from before the lifecycle comes back (a save's load, Stop's
    /// restore, an Undo).
    Back,
    /// The value is gone: back to its default, or the loaded document's.
    Gone,
    /// The lifecycle leaves the live value as it is.
    Left,
    /// The lifecycle owes it nothing (said why on the class).
    Free,
}

impl Class {
    /// What `lifecycle` promises this class.
    pub fn owes(self, lifecycle: Lifecycle) -> Outcome {
        use Lifecycle::{Load, PlayLoad, Save, Stop, Undo};
        use Outcome::{Back, Free, Gone, Left};
        match (self, lifecycle) {
            (Self::Authored | Self::Entities, Save | Stop | Undo) => Back,
            (Self::BakeOutput | Self::Selection, Save | Stop) => Back,
            (Self::BakeOutput | Self::Selection, Undo) => Left,
            (Self::Runtime, Save | Stop) => Gone,
            // An edit-mode load finds them empty: leaving Play cleared them
            // (`Scene::set_playing`).
            (Self::Runtime, Load) => Free,
            // A Play load leaves the play state and the phase's own request (it took
            // the load before the swap); what it keys by entity is `promise`'s. The
            // shader clocks run on, and the disk record stays the edit scene's,
            // which Stop puts back: a Play load reads its file and records nothing.
            (Self::Runtime | Self::Frame | Self::DiskRecord, PlayLoad) => Left,
            (Self::EditRuntime | Self::Identity | Self::DiskRecord, Save) => Gone,
            (Self::EditRuntime | Self::Identity | Self::DiskRecord, Stop) => Back,
            (Self::Frame, Save) => Gone,
            (Self::Frame, Stop | Load) => Left,
            (Self::Follow, Save) => Gone,
            (Self::Follow, Stop | Load | PlayLoad) => Free,
            // Play holds none: entering it forgets the history.
            (Self::History, Save | Stop | Load | PlayLoad) => Gone,
            // Undo works the history itself.
            (Self::History, Undo) => Free,
            (_, Undo) => Left,
            (_, Load | PlayLoad) => Gone,
        }
    }

    /// Whether a save writes it into the scene document (`SceneData`); a probe's
    /// baked SH goes to the sidecar beside it, through its field's document.
    pub fn saved(self) -> bool {
        self.owes(Lifecycle::Save) == Outcome::Back
    }
}

/// What `lifecycle` promises a value of `class`, keyed as `keyed`: its class's
/// promise, except that a Play load drops every entry keyed by an entity it
/// leaves behind, whatever the class (the old id may name an incoming entity).
pub fn promise(class: Class, keyed: Keyed, lifecycle: Lifecycle) -> Outcome {
    match (keyed, lifecycle) {
        (Keyed::Dropped | Keyed::Restored, Lifecycle::PlayLoad) => Outcome::Gone,
        _ => class.owes(lifecycle),
    }
}

/// How a field keyed by entity id follows its entities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyed {
    /// Not keyed by entity.
    No,
    /// Its entries for despawned entities are dropped (`Scene::despawn_batch`).
    Dropped,
    /// Dropped on despawn, and put back with its entity when the Destroy is undone.
    Restored,
}

/// One field of [`Scene`], classified.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// The field's name on `Scene`.
    pub name: &'static str,
    pub class: Class,
    pub keyed: Keyed,
    /// The `SceneData` fields it saves as, when not its own name.
    pub saved_as: &'static [&'static str],
}

impl Field {
    /// The `SceneData` fields a save writes it to: none for an unsaved class.
    pub fn document_names(&self) -> Vec<&'static str> {
        match (self.class.saved(), self.saved_as) {
            (false, _) => Vec::new(),
            (true, []) => vec![self.name],
            (true, names) => names.to_vec(),
        }
    }
}
