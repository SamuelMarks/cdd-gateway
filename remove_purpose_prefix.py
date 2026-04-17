import os

files = [
    'ARCHITECTURE.md',
    'CDD_CTL.md',
    'DEPLOYMENT.md',
    'DEVELOPING.md',
    'README.md',
    'USAGE.md',
    'WASM.md'
]

for filename in files:
    if not os.path.exists(filename):
        continue
        
    with open(filename, 'r', encoding='utf-8') as f:
        content = f.read()
        
    # Replace "> **Purpose:** " with "> "
    if "> **Purpose:** " in content:
        content = content.replace("> **Purpose:** ", "> ")
        with open(filename, 'w', encoding='utf-8') as f:
            f.write(content)

