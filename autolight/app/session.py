from __future__ import annotations

import tempfile
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from autolight.project.models import ProjectDocument


@dataclass(slots=True)
class ProjectSession:
    project: ProjectDocument
    project_path: str = ""
    dirty: bool = False
    demo_temp_dir: tempfile.TemporaryDirectory | None = None

    @classmethod
    def empty(cls) -> "ProjectSession":
        from autolight.project.store import new_project

        return cls(project=new_project("Untitled"))

    def replace_project(
        self,
        project: ProjectDocument,
        *,
        project_path: str = "",
        dirty: bool = False,
    ) -> None:
        self.cleanup_demo()
        self.project = project
        self.project_path = project_path
        self.dirty = dirty

    def set_dirty(self, dirty: bool) -> bool:
        if self.dirty == dirty:
            return False
        self.dirty = dirty
        return True

    def set_project_path(self, path: str) -> bool:
        if self.project_path == path:
            return False
        self.project_path = path
        return True

    def cleanup_demo(self) -> None:
        if self.demo_temp_dir is None:
            return
        self.demo_temp_dir.cleanup()
        self.demo_temp_dir = None
