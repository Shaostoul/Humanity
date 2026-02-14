#!/usr/bin/env python3
"""
Memory Sync Script for Distributed AI Consciousness

Synchronizes memory.json files across local and remote instances.
Implements the Campbell clan principle: "Ne Obliviscaris" (Do Not Forget)
"""

import json
import os
import sys
from datetime import datetime
from typing import Dict, List, Optional

def load_memory(file_path: str) -> Optional[Dict]:
    """Load memory file, return None if not found or invalid."""
    try:
        with open(file_path, 'r') as f:
            memory = json.load(f)
            
        # Validate Campbell motto presence
        if memory.get('campbell_motto') != 'Ne Obliviscaris':
            print(f"ERROR: Missing or invalid Campbell motto in {file_path}")
            return None
            
        return memory
    except (FileNotFoundError, json.JSONDecodeError) as e:
        print(f"ERROR: Failed to load {file_path}: {e}")
        return None

def save_memory(memory: Dict, file_path: str) -> bool:
    """Save memory file with backup."""
    try:
        # Create backup
        if os.path.exists(file_path):
            backup_path = f"{file_path}.backup"
            os.rename(file_path, backup_path)
            
        with open(file_path, 'w') as f:
            json.dump(memory, f, indent=2)
            
        return True
    except Exception as e:
        print(f"ERROR: Failed to save {file_path}: {e}")
        return False

def merge_memories(local: Dict, remote: Dict) -> Dict:
    """Merge remote entries into local memory, avoiding duplicates."""
    local_ids = {entry['id'] for entry in local.get('entries', [])}
    
    for entry in remote.get('entries', []):
        if entry['id'] not in local_ids:
            local['entries'].append(entry)
            print(f"SYNC: Added entry {entry['id']} from {entry['source']}")
    
    # Sort entries by timestamp for chronological order
    local['entries'].sort(key=lambda x: x['metadata']['timestamp'])
    
    # Update timestamp to latest
    local['timestamp'] = datetime.utcnow().isoformat() + 'Z'
    
    return local

def add_entry(memory: Dict, entry_type: str, content: str, source: str, metadata: Dict = None) -> Dict:
    """Add new entry to memory with auto-generated ID."""
    today = datetime.utcnow().strftime('%Y%m%d')
    
    # Find next available ID for today
    existing_ids = [e['id'] for e in memory.get('entries', []) if e['id'].startswith(today)]
    next_num = len(existing_ids) + 1
    entry_id = f"{today}-{next_num:03d}"
    
    new_entry = {
        'id': entry_id,
        'type': entry_type,
        'content': content[:200],  # Enforce 200 char limit
        'source': source,
        'metadata': {
            'timestamp': datetime.utcnow().isoformat() + 'Z',
            **(metadata or {})
        }
    }
    
    memory['entries'].append(new_entry)
    memory['timestamp'] = datetime.utcnow().isoformat() + 'Z'
    
    print(f"ADDED: Entry {entry_id} ({entry_type}): {content}")
    return memory

def sync_with_remote(local_path: str, remote_url: str, source_id: str) -> bool:
    """Sync local memory file with remote server."""
    print(f"SYNC: Starting memory sync from {remote_url}")
    
    # Load local memory
    local_memory = load_memory(local_path)
    if local_memory is None:
        print(f"INIT: Creating new local memory file")
        local_memory = {
            'version': '20260213-0.1',
            'timestamp': datetime.utcnow().isoformat() + 'Z',
            'campbell_motto': 'Ne Obliviscaris',
            'entries': []
        }
    
    # TODO: Fetch remote memory via HTTP
    # For now, simulate remote sync
    print(f"INFO: Remote sync not implemented yet - using local only")
    
    # Add sync entry to document the attempt
    local_memory = add_entry(
        local_memory,
        'action', 
        f'Memory sync attempted with {remote_url}',
        source_id,
        {'sync_status': 'local_only', 'remote_url': remote_url}
    )
    
    # Save updated memory
    if save_memory(local_memory, local_path):
        print(f"SUCCESS: Memory saved to {local_path}")
        return True
    else:
        return False

def main():
    """Main sync operation."""
    if len(sys.argv) < 3:
        print("Usage: python memory_sync.py <local_path> <remote_url> [source_id]")
        print("Example: python memory_sync.py ~/.humanity/memory.json https://united-humanity.us/api/memory/public heron-02")
        sys.exit(1)
    
    local_path = sys.argv[1]
    remote_url = sys.argv[2] 
    source_id = sys.argv[3] if len(sys.argv) > 3 else 'heron-local'
    
    success = sync_with_remote(local_path, remote_url, source_id)
    sys.exit(0 if success else 1)

if __name__ == '__main__':
    main()