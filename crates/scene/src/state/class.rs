#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Authored,
    BakeOutput,
    Entities,
    Selection,
    Runtime,
    EditRuntime,
    Frame,
    Identity,
    DiskRecord,
    Follow,
    History,
    Request,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Save,
    Stop,
    Undo,
    Load,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Back,
    Gone,
    Left,
    Free,
}

impl Class {
    pub fn owes(self, lifecycle: Lifecycle) -> Outcome {
        use Lifecycle::{Load, PlayLoad, Save, Stop, Undo};
        use Outcome::{Back, Free, Gone, Left};
        match (self, lifecycle) {
            (Self::Authored | Self::Entities, Save | Stop | Undo) => Back,
            (Self::BakeOutput | Self::Selection, Save | Stop) => Back,
            (Self::BakeOutput | Self::Selection, Undo) => Left,
            (Self::Runtime, Save | Stop) => Gone,
            (Self::Runtime, Load) => Free,
            (Self::Runtime | Self::Frame | Self::DiskRecord, PlayLoad) => Left,
            (Self::EditRuntime | Self::Identity | Self::DiskRecord, Save) => Gone,
            (Self::EditRuntime | Self::Identity | Self::DiskRecord, Stop) => Back,
            (Self::Frame, Save) => Gone,
            (Self::Frame, Stop | Load) => Left,
            (Self::Follow, Save) => Gone,
            (Self::Follow, Stop | Load | PlayLoad) => Free,
            (Self::History, Save | Stop | Load | PlayLoad) => Gone,
            (Self::History, Undo) => Free,
            (Self::Request, Save | Stop) => Gone,
            (_, Undo) => Left,
            (_, Load | PlayLoad) => Gone,
        }
    }

    pub fn saved(self) -> bool {
        self.owes(Lifecycle::Save) == Outcome::Back
    }
}

pub fn promise(class: Class, keyed: Keyed, lifecycle: Lifecycle) -> Outcome {
    match (keyed, lifecycle) {
        (Keyed::Dropped | Keyed::Restored, Lifecycle::PlayLoad) => Outcome::Gone,
        _ => class.owes(lifecycle),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyed {
    No,
    Dropped,
    Restored,
    Checked,
}

#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: &'static str,
    pub class: Class,
    pub keyed: Keyed,
    pub saved_as: &'static [&'static str],
}

impl Field {
    pub fn document_names(&self) -> Vec<&'static str> {
        match (self.class.saved(), self.saved_as) {
            (false, _) => Vec::new(),
            (true, []) => vec![self.name],
            (true, names) => names.to_vec(),
        }
    }
}
