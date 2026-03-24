USE NS project_universe;
USE DB humanities;

DEFINE TABLE sociology SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE sociology TYPE string;
DEFINE FIELD description     ON TABLE sociology TYPE string;
DEFINE FIELD type            ON TABLE sociology TYPE string;
DEFINE FIELD theories        ON TABLE sociology TYPE array;
DEFINE FIELD applications    ON TABLE sociology TYPE array;
DEFINE FIELD limitations     ON TABLE sociology TYPE array;
DEFINE FIELD related_fields  ON TABLE sociology TYPE array;

DEFINE INDEX unique_sociology_name ON TABLE sociology COLUMNS name UNIQUE;

-- Example sociology concepts
CREATE sociology:structural_functionalism SET
  name = "Structural Functionalism",
  description = "A framework for building theory that sees society as a complex system",
  type = "Theory",
  theories = ["Social Stability", "Institutional Roles"],
  applications = ["Social Order Analysis", "Institutional Studies"],
  limitations = ["Overemphasis on Stability"],
  related_fields = ["humanities:anthropology", "humanities:politics"];

CREATE sociology:conflict_theory SET
  name = "Conflict Theory",
  description = "A theory that emphasizes the role of coercion and power in producing social order",
  type = "Theory",
  theories = ["Class Struggle", "Power Dynamics"],
  applications = ["Social Inequality Studies", "Power Structure Analysis"],
  limitations = ["Neglects Social Stability"],
  related_fields = ["humanities:history", "humanities:politics"];

CREATE sociology:symbolic_interactionism SET
  name = "Symbolic Interactionism",
  description = "A theory that focuses on the relationships among individuals within a society",
  type = "Theory",
  theories = ["Social Interaction", "Meaning-Making"],
  applications = ["Micro-Level Social Analysis", "Communication Studies"],
  limitations = ["Neglects Macro-Level Structures"],
  related_fields = ["humanities:language", "humanities:psychology"];
