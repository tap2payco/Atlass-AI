from typing import Any, Generic, TypeVar, Literal, Union
from pydantic import BaseModel, Field
from uuid import UUID
from datetime import datetime

T = TypeVar("T")

class EventEnvelope(BaseModel, Generic[T]):
    id: UUID
    timestamp: datetime
    payload: T

class AgentStarted(BaseModel):
    type: Literal["AgentStarted"] = "AgentStarted"
    data: dict[str, Any] # Contains agent_id and task_id

class AgentCompleted(BaseModel):
    type: Literal["AgentCompleted"] = "AgentCompleted"
    data: dict[str, Any]

class TaskCreated(BaseModel):
    type: Literal["TaskCreated"] = "TaskCreated"
    data: dict[str, Any]

class TaskCompleted(BaseModel):
    type: Literal["TaskCompleted"] = "TaskCompleted"
    data: dict[str, Any]

class MemoryUpdated(BaseModel):
    type: Literal["MemoryUpdated"] = "MemoryUpdated"
    data: dict[str, Any]

class ContextResolved(BaseModel):
    type: Literal["ContextResolved"] = "ContextResolved"
    data: dict[str, Any]

class ToolExecuted(BaseModel):
    type: Literal["ToolExecuted"] = "ToolExecuted"
    data: dict[str, Any]

class DecisionMade(BaseModel):
    type: Literal["DecisionMade"] = "DecisionMade"
    data: dict[str, Any]

AtlasEvent = Union[
    AgentStarted,
    AgentCompleted,
    TaskCreated,
    TaskCompleted,
    MemoryUpdated,
    ContextResolved,
    ToolExecuted,
    DecisionMade,
]
