#!/usr/bin/env python3
"""Extract meaningful conversation pairs from Claude Code session logs.

Reads JSONL session files and extracts user-assistant conversation pairs
that contain substantive technical discussions. Filters out trivial
interactions (greetings, short exchanges, tool-only responses).

Output: JSON with conversation pairs suitable for semantic knowledge extraction.
"""

import argparse
import json
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


MIN_ASSISTANT_TEXT_LENGTH = 80
MIN_USER_TEXT_LENGTH = 10


def find_session_logs():
    """Find all JSONL session logs in ~/.claude/projects/."""
    base = Path.home() / ".claude" / "projects"
    if not base.exists():
        return []
    return sorted(
        base.rglob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True
    )


def parse_entries(filepath):
    """Parse all entries from a JSONL file."""
    entries = []
    with open(filepath, "r") as f:
        for line in f:
            try:
                entry = json.loads(line.strip())
                entries.append(entry)
            except json.JSONDecodeError:
                continue
    return entries


def extract_text_content(message):
    """Extract text content from a message, ignoring tool_use/tool_result blocks."""
    content = message.get("content", [])
    if isinstance(content, str):
        return content

    texts = []
    for block in content:
        if isinstance(block, dict) and block.get("type") == "text":
            texts.append(block.get("text", ""))
    return "\n".join(texts)


def extract_user_text(message):
    """Extract text from a user message."""
    content = message.get("content", "")
    if isinstance(content, str):
        return content

    texts = []
    for block in content:
        if isinstance(block, dict):
            if block.get("type") == "tool_result":
                continue
            if block.get("type") == "text":
                texts.append(block.get("text", ""))
    return "\n".join(texts)


def has_substantive_text(assistant_message):
    """Check if an assistant message contains substantive text (not just tool calls)."""
    content = assistant_message.get("content", [])
    if isinstance(content, str):
        return len(content) >= MIN_ASSISTANT_TEXT_LENGTH

    for block in content:
        if isinstance(block, dict) and block.get("type") == "text":
            text = block.get("text", "")
            if len(text) >= MIN_ASSISTANT_TEXT_LENGTH:
                return True
    return False


def is_subagent_log(filepath):
    """Check if a log file is from a subagent (inside subagents/ directory)."""
    return "subagents" in filepath.parts


def extract_conversations_from_session(filepath, since_dt=None):
    """Extract conversation pairs from a single session log.

    Returns list of {user, assistant, session_id, timestamp} dicts.
    """
    if is_subagent_log(filepath):
        return []

    entries = parse_entries(filepath)
    if not entries:
        return []

    # Check if session is within the time window
    if since_dt:
        first_ts = None
        for entry in entries:
            ts = entry.get("timestamp")
            if ts:
                first_ts = ts
                break
        if first_ts:
            try:
                entry_dt = datetime.fromisoformat(first_ts.replace("Z", "+00:00"))
                if entry_dt < since_dt:
                    return []
            except (ValueError, TypeError):
                pass

    # Build uuid -> entry index for thread tracking
    uuid_map = {}
    for entry in entries:
        uuid = entry.get("uuid")
        if uuid:
            uuid_map[uuid] = entry

    # Extract user -> assistant pairs
    conversations = []
    session_id = None

    for entry in entries:
        if not session_id:
            session_id = entry.get("sessionId", filepath.stem)

        if entry.get("type") != "user":
            continue

        # Skip tool_result entries (these are tool outputs, not user questions)
        msg = entry.get("message", {})
        content = msg.get("content", "")
        if isinstance(content, list):
            is_tool_result = all(
                isinstance(b, dict) and b.get("type") == "tool_result"
                for b in content
                if isinstance(b, dict)
            )
            if is_tool_result:
                continue

        user_text = extract_user_text(msg)
        if len(user_text.strip()) < MIN_USER_TEXT_LENGTH:
            continue

        # Find the assistant response(s) that follow this user message
        user_uuid = entry.get("uuid")
        if not user_uuid:
            continue

        # Collect assistant responses that reference this user message
        assistant_texts = []
        for candidate in entries:
            if candidate.get("type") != "assistant":
                continue
            if candidate.get("parentUuid") != user_uuid:
                continue
            if has_substantive_text(candidate.get("message", {})):
                text = extract_text_content(candidate.get("message", {}))
                if text.strip():
                    assistant_texts.append(text)

        if not assistant_texts:
            continue

        combined_assistant = "\n".join(assistant_texts)
        timestamp = entry.get("timestamp", "")

        conversations.append(
            {
                "user": user_text.strip(),
                "assistant": combined_assistant.strip(),
                "session_id": session_id,
                "timestamp": timestamp,
            }
        )

    return conversations


def extract_all(since_days=None):
    """Extract conversations from all session logs."""
    logs = find_session_logs()
    if not logs:
        return {"error": "No session logs found", "conversations": []}

    since_dt = None
    if since_days is not None:
        since_dt = datetime.now(timezone.utc) - timedelta(days=since_days)

    all_conversations = []
    for log_path in logs:
        convs = extract_conversations_from_session(log_path, since_dt)
        all_conversations.extend(convs)

    # Sort by timestamp (newest first)
    all_conversations.sort(key=lambda c: c.get("timestamp", ""), reverse=True)

    return {
        "total_sessions_scanned": len(logs),
        "total_conversations": len(all_conversations),
        "conversations": all_conversations,
    }


def main():
    parser = argparse.ArgumentParser(
        description="Extract conversation pairs from Claude Code session logs"
    )
    parser.add_argument(
        "--since",
        type=int,
        default=None,
        metavar="DAYS",
        help="Only include conversations from the last N days",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=None,
        metavar="N",
        help="Limit output to N most recent conversations",
    )
    args = parser.parse_args()

    result = extract_all(since_days=args.since)

    if args.limit and result.get("conversations"):
        result["conversations"] = result["conversations"][: args.limit]
        result["total_conversations"] = len(result["conversations"])

    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
