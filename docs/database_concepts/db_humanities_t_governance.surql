USE NS project_universe;
USE DB humanities;

DEFINE TABLE governance SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE governance TYPE string;
DEFINE FIELD description     ON TABLE governance TYPE string;
DEFINE FIELD type            ON TABLE governance TYPE string;
DEFINE FIELD principles      ON TABLE governance TYPE array;
DEFINE FIELD applications    ON TABLE governance TYPE array;
DEFINE FIELD limitations     ON TABLE governance TYPE array;
DEFINE FIELD related_fields  ON TABLE governance TYPE array;

DEFINE INDEX unique_governance_name ON TABLE governance COLUMNS name UNIQUE;

-- Example governance systems
CREATE governance:constitutional_monarchy SET
  name = "Constitutional Monarchy",
  description = "A system of government in which a monarch shares power with a constitutionally organized government",
  type = "Government System",
  principles = ["Rule of Law", "Monarch as Head of State"],
  applications = ["National Governance", "Ceremonial Duties"],
  limitations = ["Potential for Power Imbalance"],
  related_fields = ["humanities:politics", "humanities:law"];

CREATE governance:republic SET
  name = "Republic",
  description = "A form of government in which the country is considered a "public matter", and is not the private concern or property of the rulers",
  type = "Government System",
  principles = ["Elected Representatives", "Public Sovereignty"],
  applications = ["National Governance", "Public Administration"],
  limitations = ["Potential for Political Instability"],
  related_fields = ["humanities:politics", "humanities:law"];

CREATE governance:anarchy SET
  name = "Anarchy",
  description = "A state of society without government or law",
  type = "Political Philosophy",
  principles = ["Absence of Government", "Self-Governance"],
  applications = ["Social Movements", "Political Theory"],
  limitations = ["Potential for Disorder"],
  related_fields = ["humanities:politics", "humanities:sociology"];
