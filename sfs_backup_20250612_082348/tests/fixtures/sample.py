# Python sample file for testing
import os
from typing import Optional, List

class DataProcessor:
    def __init__(self, name: str):
        self.name = name
        self._data = []

    def add_item(self, item: str) -> None:
        """Add an item to the processor."""
        self._data.append(item)

    def process_data(self) -> List[str]:
        """Process all data items."""
        return [item.upper() for item in self._data]

    @property
    def item_count(self) -> int:
        return len(self._data)

    @staticmethod
    def create_default():
        return DataProcessor("default")

def process_file(filename: str) -> Optional[str]:
    """Process a file and return its content."""
    if not os.path.exists(filename):
        return None
    
    with open(filename, 'r') as f:
        return f.read()

def calculate_sum(*args):
    return sum(args)

# Module constants
MAX_ITEMS = 1000
DEFAULT_ENCODING = "utf-8"

# Global variables
_cache = {}
counter = 0