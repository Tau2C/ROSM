//! `rosm_core`: Core OpenStreetMap data structures, indexing, command history, and validation.

pub mod command;
pub mod dataset;
pub mod geo;
pub mod id;
pub mod primitive;
pub mod tags;
pub mod validator;

// Convenient re-exports
pub use command::{
    AddPrimitiveCommand, ChangeTagsCommand, CombineWaysCommand, Command, CommandError,
    CommandStack, DeletePrimitiveCommand, MergeNodesCommand, ModifyWayNodesCommand,
    MoveNodeCommand, ReverseWayCommand, SequenceCommand, SplitWayCommand,
};
pub use dataset::DataSet;
pub use geo::{BoundingBox, LatLon};
pub use id::{NodeId, PrimitiveId, RelationId, WayId};
pub use primitive::{Node, OsmMetadata, OsmPrimitive, Relation, RelationMember, Way};
pub use tags::{TagDiff, Tags};
pub use validator::{validate, Severity, ValidationIssue};
