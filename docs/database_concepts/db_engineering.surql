USE NS project_universe;

DEFINE DATABASE engineering;

USE DB engineering;

DEFINE TABLE engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE engineering TYPE string;
DEFINE FIELD description     ON TABLE engineering TYPE string;
DEFINE FIELD type            ON TABLE engineering TYPE string;
DEFINE FIELD disciplines     ON TABLE engineering TYPE array;
DEFINE FIELD applications    ON TABLE engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE engineering TYPE array;

DEFINE INDEX unique_engineering_name ON TABLE engineering COLUMNS name UNIQUE;

-- Example engineering concepts
CREATE engineering:civil_engineering SET
  name = "Civil Engineering",
  description = "A professional engineering discipline that deals with the design, construction, and maintenance of the physical and naturally built environment",
  type = "Discipline",
  disciplines = ["Structural Engineering", "Environmental Engineering"],
  applications = ["Infrastructure Development", "Urban Planning"],
  related_fields = ["engineering:architecture", "engineering:construction"];

CREATE engineering:electrical_engineering SET
  name = "Electrical Engineering",
  description = "A field of engineering that generally deals with the study and application of electricity, electronics, and electromagnetism",
  type = "Discipline",
  disciplines = ["Power Systems", "Control Systems"],
  applications = ["Power Generation", "Electronics Design"],
  related_fields = ["engineering:electronics", "engineering:telecommunications"];

CREATE engineering:mechanical_engineering SET
  name = "Mechanical Engineering",
  description = "An engineering branch that combines engineering physics and mathematics principles with materials science to design, analyze, manufacture, and maintain mechanical systems",
  type = "Discipline",
  disciplines = ["Thermodynamics", "Fluid Mechanics"],
  applications = ["Automotive Design", "Aerospace Engineering"],
  related_fields = ["engineering:manufacturing", "engineering:robotics"];
