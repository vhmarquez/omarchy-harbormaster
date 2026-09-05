"""Exercise installed Hermes' real loader with explicitly synthetic callbacks.

No AIAgent is constructed. These are not live session/tool/approval observations.
"""
import json
import sys
from pathlib import Path

sys.path[:0] = ["/opt/hermes", "/opt/spike"]
from hermes_cli.plugins import PluginManager, VALID_HOOKS
from observe import EVENTS

manager = PluginManager()
manager.discover_and_load()
results = {}
for event in sorted(EVENTS["hermes"]):
    assert event in VALID_HOOKS, event
    results[event] = manager.invoke_hook(
        event,
        session_id="fixture-session", turn_id="fixture-turn",
        parent_session_id="fixture-parent", child_session_id="fixture-child",
        parent_turn_id="fixture-parent-turn", child_subagent_id="fixture-child-agent",
        surface="cli", command="SYNTHETIC_SECRET", description="SYNTHETIC_SECRET",
        child_goal="SYNTHETIC_SECRET", user_message="SYNTHETIC_SECRET",
        response="SYNTHETIC_SECRET", transcript_path="/must/not/open",
    )
assert all(values == [] for values in results.values()), "observer returned a directive"
print(json.dumps({"kind": "installed-loader-synthetic-dispatch", "hook_results": results,
                  "plugins": [p for p in manager.list_plugins() if p.get("name", "").startswith("harbormaster-m0")]}))
