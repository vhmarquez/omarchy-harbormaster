"""Scoped Rust/QML lexical metrics; not a language parser.

Kept separate from filesystem/report policy after the reporter crossed its
300-nonblank-line review threshold. Named function spans share one scanner.
"""
import re


def comment_end(text, start):
    depth = 1
    for match in re.finditer(r"/\*|\*/", text[start + 2:]):
        depth += 1 if match[0] == "/*" else -1
        if depth == 0:
            return start + 2 + match.end()
    raise ValueError("unterminated block comment")


def mask_literals(text, language):
    """Blank comments/literals while preserving character offsets and newlines."""
    pattern = r'//[^\n]*|/\*|"'
    if language == "rust":
        pattern = r'\b(?:br|cr|r)(?P<hashes>#{0,255})"|' + pattern
        pattern += r"|'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'"
    else:
        pattern += "|'|`"
    token = re.compile(pattern)
    masked = list(text)
    position = 0
    while match := token.search(text, position):
        start, end = match.span()
        if match[0] == "/*":
            end = comment_end(text, start)
        elif language == "rust" and match.group("hashes") is not None:
            terminator = '"' + match.group("hashes")
            stop = text.find(terminator, end)
            if stop < 0:
                raise ValueError("unterminated raw string")
            end = stop + len(terminator)
        elif match[0] in {'"', "'", "`"}:
            quote = re.escape(match[0])
            pattern = quote + r"(?:\\.|[^" + match[0] + r"\\])*" + quote
            literal = re.match(pattern, text[start:], re.S)
            if literal is None:
                raise ValueError("unterminated string")
            end = start + literal.end()
        masked[start:end] = re.sub(r"[^\n]", " ", text[start:end])
        position = end
    return "".join(masked)


def brace_pairs(text):
    stack, pairs = [], {}
    for match in re.finditer(r"[{}]", text):
        if match[0] == "{":
            stack.append(match.start())
        elif stack:
            pairs[stack.pop()] = match.start()
        else:
            raise ValueError("unmatched closing brace")
    if stack:
        raise ValueError("unmatched opening brace")
    return pairs


def body_opening(code, start):
    depth = 0
    for match in re.finditer(r"[()\[\]{};]", code[start:]):
        token = match[0]
        if token in "([":
            depth += 1
        elif token in ")]":
            depth -= 1
        elif depth == 0:
            return start + match.start() if token == "{" else None
    return None


def lexical_functions(text, language):
    code = mask_literals(text, language)
    pairs = brace_pairs(code)
    keyword = "fn" if language == "rust" else "function"
    spans = []
    for match in re.finditer(r"\b" + keyword + r"\s+((?:r#)?[A-Za-z_]\w*)", code):
        opening = body_opening(code, match.end())
        if opening in pairs:
            spans.append((match.start(), opening, pairs[opening], match[1]))
    found = []
    for start, opening, end, name in spans:
        body = list(code[opening + 1:end])
        parents = []
        for other, _, stop, parent_name in spans:
            if other < start < stop:
                parents.append(parent_name)
            if opening < other < end:
                body[other - opening - 1:stop - opening] = " " * (stop - other + 1)
        body = "".join(body)
        depth = maximum = 0
        for brace in re.findall(r"[{}]", body):
            depth += 1 if brace == "{" else -1
            maximum = max(maximum, depth)
        line = text.count("\n", 0, start) + 1
        end_line = text.count("\n", 0, end) + 1
        found.append({
            "name": ".".join(parents + [name]), "line": line, "end_line": end_line,
            "nonblank_lines": sum(bool(value.strip()) for value in text.splitlines()[line - 1:end_line]),
            "nesting": maximum,
            "branches": len(re.findall(r"\b(?:if|for|while|loop|match|switch|case|catch)\b|&&|\|\||\?", body)),
        })
    return found
