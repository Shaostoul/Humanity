USE NS project_universe;
USE DB engineering;

DEFINE TABLE software_engineering SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD name            ON TABLE software_engineering TYPE string;
DEFINE FIELD description     ON TABLE software_engineering TYPE string;
DEFINE FIELD type            ON TABLE software_engineering TYPE string;
DEFINE FIELD methodologies   ON TABLE software_engineering TYPE array;
DEFINE FIELD applications    ON TABLE software_engineering TYPE array;
DEFINE FIELD related_fields  ON TABLE software_engineering TYPE array;

DEFINE INDEX unique_software_engineering_name ON TABLE software_engineering COLUMNS name UNIQUE;

-- Example software engineering concepts
CREATE software_engineering:agile_development SET
  name = "Agile Development",
  description = "A set of principles for software development under which requirements and solutions evolve through the collaborative effort of cross-functional teams",
  type = "Methodology",
  methodologies = ["Scrum", "Kanban"],
  applications = ["Software Development", "Project Management"],
  related_fields = ["engineering:project_management", "engineering:team_collaboration"];

CREATE software_engineering:devops SET
  name = "DevOps",
  description = "A set of practices that combines software development (Dev) and IT operations (Ops) to shorten the systems development life cycle",
  type = "Practice",
  methodologies = ["Continuous Integration", "Continuous Deployment"],
  applications = ["Software Deployment", "Infrastructure Management"],
  related_fields = ["engineering:it_operations", "engineering:cloud_computing"];

CREATE software_engineering:software_testing SET
  name = "Software Testing",
  description = "The process of evaluating and verifying that a software product or application does what it is supposed to do",
  type = "Process",
  methodologies = ["Unit Testing", "Integration Testing"],
  applications = ["Quality Assurance", "Bug Detection"],
  related_fields = ["engineering:quality_assurance", "engineering:software_development"];
