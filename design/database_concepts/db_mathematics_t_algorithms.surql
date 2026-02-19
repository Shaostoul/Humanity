USE NS project_universe;
USE DB mathematics;

DEFINE TABLE algorithms SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE algorithms TYPE string;
DEFINE FIELD description     ON TABLE algorithms TYPE string;
DEFINE FIELD type            ON TABLE algorithms TYPE string;
DEFINE FIELD complexity      ON TABLE algorithms TYPE string;
DEFINE FIELD applications    ON TABLE algorithms TYPE array;
DEFINE FIELD limitations     ON TABLE algorithms TYPE array;
DEFINE FIELD related_fields  ON TABLE algorithms TYPE array;

DEFINE INDEX unique_algorithm_name ON TABLE algorithms COLUMNS name UNIQUE;

-- Example algorithms
CREATE algorithms:quick_sort SET
  name = "Quick Sort",
  description = "Efficient sorting algorithm using divide and conquer",
  type = "Sorting",
  complexity = "O(n log n)",
  applications = ["Data Sorting", "Search Optimization"],
  limitations = ["Worst-case O(n²) complexity"],
  related_fields = ["mathematics:discrete_math", "computer_science:data_structures"];

CREATE algorithms:dijkstra SET
  name = "Dijkstra's Algorithm",
  description = "Algorithm for finding the shortest paths between nodes in a graph",
  type = "Graph",
  complexity = "O(V²) or O(E + V log V) with priority queue",
  applications = ["Network Routing", "Map Navigation"],
  limitations = ["Does not work with negative weights"],
  related_fields = ["mathematics:graph_theory", "computer_science:networking"];

CREATE algorithms:fft SET
  name = "Fast Fourier Transform",
  description = "Algorithm to compute the discrete Fourier transform and its inverse",
  type = "Transform",
  complexity = "O(n log n)",
  applications = ["Signal Processing", "Image Compression"],
  limitations = ["Requires power of two input size"],
  related_fields = ["mathematics:signal_processing", "computer_science:audio_processing"];

CREATE algorithms:neural_network SET
  name = "Neural Network",
  description = "Computational model inspired by the human brain, used in machine learning",
  type = "Machine Learning",
  complexity = "Varies with architecture",
  applications = ["Image Recognition", "Natural Language Processing"],
  limitations = ["Requires large datasets", "Computationally intensive"],
  related_fields = ["mathematics:statistics", "computer_science:ai"];
