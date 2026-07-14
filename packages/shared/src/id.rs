use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}_{}", stringify!($name).to_lowercase().replace("id", ""), self.0)
            }
        }
    };
}

define_id!(WorkspaceId);
define_id!(ProjectId);
define_id!(SessionId);
define_id!(TaskId);
define_id!(AgentId);
define_id!(WorkflowId);
define_id!(MemoryId);
define_id!(RepositoryId);
define_id!(ExecutionId);
define_id!(ToolId);
