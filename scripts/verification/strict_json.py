"""Shared structural JSON policy; callers retain their own byte limits and I/O."""
import json
import math


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate evidence key")
        result[key] = value
    return result


def loads(data):
    """Decode UTF-8 JSON, rejecting duplicates, nonfinite values and deep trees."""
    if isinstance(data, bytes):
        data = data.decode("utf-8")
    try:
        result = json.loads(data, object_pairs_hook=_unique)
    except RecursionError as error:
        raise ValueError("evidence JSON nesting exceeds limit") from error
    pending = [(result, 0)]
    while pending:
        value, depth = pending.pop()
        if depth > 64:
            raise ValueError("evidence JSON nesting exceeds limit")
        if isinstance(value, float) and not math.isfinite(value):
            raise ValueError("evidence JSON contains a nonfinite number")
        if isinstance(value, str):
            value.encode("utf-8")  # Reject unpaired surrogates, including object keys.
        elif isinstance(value, dict):
            pending.extend((child, depth + 1) for child in value.keys())
            pending.extend((child, depth + 1) for child in value.values())
        elif isinstance(value, list):
            pending.extend((child, depth + 1) for child in value)
    return result
