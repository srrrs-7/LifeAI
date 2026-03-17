#!/usr/bin/env python3
"""Claude Code session log analyzer.

Reads JSONL session files and outputs a structured JSON report with:
- Token usage summary (input, output, cache)
- Tool usage frequency
- Error/failure patterns
- Session metadata
- Model usage breakdown
"""

import json
import sys
import os
import glob
from collections import Counter, defaultdict
from pathlib import Path


def find_session_logs():
    """Find all JSONL session logs in ~/.claude/projects/."""
    base = Path.home() / ".claude" / "projects"
    if not base.exists():
        return []
    return sorted(base.rglob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)


def analyze_session(filepath):
    """Analyze a single session JSONL file."""
    stats = {
        "file": str(filepath),
        "message_count": 0,
        "user_messages": 0,
        "assistant_messages": 0,
        "tokens": {
            "total_input": 0,
            "total_output": 0,
            "total_cache_creation": 0,
            "total_cache_read": 0,
        },
        "tools_used": Counter(),
        "tool_errors": [],
        "models_used": Counter(),
        "errors": [],
        "timestamps": [],
    }

    with open(filepath, "r") as f:
        for line in f:
            try:
                entry = json.loads(line.strip())
            except json.JSONDecodeError:
                continue

            entry_type = entry.get("type", "")
            timestamp = entry.get("timestamp")
            if timestamp:
                stats["timestamps"].append(timestamp)

            if entry_type == "user":
                stats["user_messages"] += 1
                stats["message_count"] += 1

            elif entry_type == "assistant":
                stats["assistant_messages"] += 1
                stats["message_count"] += 1

                msg = entry.get("message", {})
                model = msg.get("model", "unknown")
                stats["models_used"][model] += 1

                usage = msg.get("usage", {})
                stats["tokens"]["total_input"] += usage.get("input_tokens", 0)
                stats["tokens"]["total_output"] += usage.get("output_tokens", 0)
                stats["tokens"]["total_cache_creation"] += usage.get("cache_creation_input_tokens", 0)
                stats["tokens"]["total_cache_read"] += usage.get("cache_read_input_tokens", 0)

                # Extract tool usage from content blocks
                content = msg.get("content", [])
                for block in content:
                    if isinstance(block, dict):
                        if block.get("type") == "tool_use":
                            tool_name = block.get("name", "unknown")
                            stats["tools_used"][tool_name] += 1
                        elif block.get("type") == "tool_result":
                            if block.get("is_error"):
                                stats["tool_errors"].append({
                                    "tool": block.get("tool_use_id", "unknown"),
                                    "content": str(block.get("content", ""))[:200],
                                })

            elif entry_type == "system":
                msg = entry.get("message", {})
                content = msg.get("content", "")
                if isinstance(content, str) and "error" in content.lower():
                    stats["errors"].append(content[:200])

    # Convert Counters to dicts for JSON serialization
    stats["tools_used"] = dict(stats["tools_used"].most_common())
    stats["models_used"] = dict(stats["models_used"].most_common())

    # Calculate duration if timestamps available
    if len(stats["timestamps"]) >= 2:
        stats["first_timestamp"] = stats["timestamps"][0]
        stats["last_timestamp"] = stats["timestamps"][-1]
    del stats["timestamps"]

    return stats


def analyze_all():
    """Analyze all session logs and produce aggregate report."""
    logs = find_session_logs()

    if not logs:
        return {"error": "No session logs found", "sessions": []}

    sessions = []
    aggregate = {
        "total_sessions": len(logs),
        "total_messages": 0,
        "total_tokens": {
            "input": 0,
            "output": 0,
            "cache_creation": 0,
            "cache_read": 0,
        },
        "tool_frequency": Counter(),
        "model_frequency": Counter(),
        "all_errors": [],
        "all_tool_errors": [],
    }

    for log_path in logs:
        session = analyze_session(log_path)
        sessions.append(session)

        aggregate["total_messages"] += session["message_count"]
        aggregate["total_tokens"]["input"] += session["tokens"]["total_input"]
        aggregate["total_tokens"]["output"] += session["tokens"]["total_output"]
        aggregate["total_tokens"]["cache_creation"] += session["tokens"]["total_cache_creation"]
        aggregate["total_tokens"]["cache_read"] += session["tokens"]["total_cache_read"]

        for tool, count in session["tools_used"].items():
            aggregate["tool_frequency"][tool] += count
        for model, count in session["models_used"].items():
            aggregate["model_frequency"][model] += count

        aggregate["all_errors"].extend(session["errors"])
        aggregate["all_tool_errors"].extend(session["tool_errors"])

    # Calculate cache efficiency
    total_cache = aggregate["total_tokens"]["cache_creation"] + aggregate["total_tokens"]["cache_read"]
    total_input = aggregate["total_tokens"]["input"] + total_cache
    aggregate["cache_hit_rate"] = (
        round(aggregate["total_tokens"]["cache_read"] / total_cache * 100, 1)
        if total_cache > 0 else 0
    )
    aggregate["cache_usage_rate"] = (
        round(total_cache / total_input * 100, 1)
        if total_input > 0 else 0
    )

    aggregate["tool_frequency"] = dict(aggregate["tool_frequency"].most_common())
    aggregate["model_frequency"] = dict(aggregate["model_frequency"].most_common())

    return {
        "aggregate": aggregate,
        "sessions": sessions,
    }


if __name__ == "__main__":
    report = analyze_all()
    print(json.dumps(report, indent=2, ensure_ascii=False))
