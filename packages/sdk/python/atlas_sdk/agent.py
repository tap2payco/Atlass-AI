from abc import ABC, abstractmethod
from typing import Any
from uuid import UUID

class AgentRuntime(ABC):
    @abstractmethod
    async def invoke(self, agent_id: UUID, task_id: UUID, context: str) -> str:
        """Invoke the agent with the given context."""
        pass
        
    @abstractmethod
    async def get_status(self, agent_id: UUID) -> str:
        """Get the current status of the agent."""
        pass
        
    @abstractmethod
    async def terminate(self, agent_id: UUID) -> None:
        """Terminate the agent execution."""
        pass
