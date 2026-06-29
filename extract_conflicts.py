import os
import glob

def find_conflicts(directory):
    for root, _, files in os.walk(directory):
        for file in files:
            if not file.endswith('.rs') and not file.endswith('.toml') and not file.endswith('.md'):
                continue
            path = os.path.join(root, file)
            try:
                with open(path, 'r', encoding='utf-8') as f:
                    content = f.read()
            except:
                continue
            
            if '<<<<<<< HEAD' in content:
                print(f"=== {path} ===")
                lines = content.splitlines()
                in_conflict = False
                conflict_block = []
                for line in lines:
                    if line.startswith('<<<<<<<'):
                        in_conflict = True
                        conflict_block = [line]
                    elif line.startswith('======='):
                        conflict_block.append(line)
                    elif line.startswith('>>>>>>>'):
                        conflict_block.append(line)
                        in_conflict = False
                        print('\n'.join(conflict_block))
                        print('-' * 40)
                    elif in_conflict:
                        conflict_block.append(line)

if __name__ == '__main__':
    find_conflicts('.')
