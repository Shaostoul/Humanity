USE NS project_universe;
USE DB humanities;

DEFINE TABLE politics SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE politics TYPE string;
DEFINE FIELD description     ON TABLE politics TYPE string;
DEFINE FIELD type            ON TABLE politics TYPE string;
DEFINE FIELD ideologies      ON TABLE politics TYPE array;
DEFINE FIELD applications    ON TABLE politics TYPE array;
DEFINE FIELD limitations     ON TABLE politics TYPE array;
DEFINE FIELD related_fields  ON TABLE politics TYPE array;

DEFINE INDEX unique_politics_name ON TABLE politics COLUMNS name UNIQUE;

-- Example political concepts
CREATE politics:democracy SET
  name = "Democracy",
  description = "A system of government by the whole population, typically through elected representatives",
  type = "Government System",
  ideologies = ["Popular Sovereignty", "Electoral Process"],
  applications = ["National Governance", "Local Governance"],
  limitations = ["Potential for Majority Tyranny"],
  related_fields = ["humanities:history", "humanities:law"];

CREATE politics:authoritarianism SET
  name = "Authoritarianism",
  description = "A form of government characterized by strong central power and limited political freedoms",
  type = "Government System",
  ideologies = ["Centralized Control", "Limited Political Freedoms"],
  applications = ["State Governance", "Political Stability"],
  limitations = ["Suppression of Dissent"],
  related_fields = ["humanities:history", "humanities:law"];

CREATE politics:libertarianism SET
  name = "Libertarianism",
  description = "A political philosophy advocating only minimal state intervention in the lives of citizens",
  type = "Political Philosophy",
  ideologies = ["Individual Liberty", "Free Market"],
  applications = ["Economic Policy", "Civil Liberties"],
  limitations = ["Potential for Inequality"],
  related_fields = ["humanities:economics", "humanities:law"];
