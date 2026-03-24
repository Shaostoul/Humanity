USE NS project_universe;
USE DB game;

DEFINE TABLE moral_agents SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD agent_name    ON TABLE moral_agents TYPE string;
DEFINE FIELD agent_type    ON TABLE moral_agents TYPE string;
DEFINE FIELD description   ON TABLE moral_agents TYPE string;

DEFINE INDEX unique_agent_name ON TABLE moral_agents COLUMNS agent_name UNIQUE;

-- Individual Moral Agents
CREATE moral_agents:human_individual SET
  agent_name = "Human Individual",
  agent_type = "Individual",
  description = "A single human being capable of making moral decisions.";

CREATE moral_agents:religious_leader SET
  agent_name = "Religious Leader",
  agent_type = "Individual",
  description = "A spiritual guide who provides moral direction to their community.";

CREATE moral_agents:philosopher SET
  agent_name = "Philosopher",
  agent_type = "Individual",
  description = "A thinker who examines and develops moral theories and principles.";

CREATE moral_agents:activist SET
  agent_name = "Social Activist",
  agent_type = "Individual",
  description = "A person who actively works to promote social change and justice.";

-- Organizational Moral Agents
CREATE moral_agents:corporation SET
  agent_name = "Corporation",
  agent_type = "Organization",
  description = "A business entity with collective moral responsibility.";

CREATE moral_agents:non_profit SET
  agent_name = "Non-Profit Organization",
  agent_type = "Organization",
  description = "A charitable organization dedicated to social causes.";

CREATE moral_agents:educational_institution SET
  agent_name = "Educational Institution",
  agent_type = "Organization",
  description = "An organization responsible for moral education and character development.";

CREATE moral_agents:religious_organization SET
  agent_name = "Religious Organization",
  agent_type = "Organization",
  description = "A faith-based group that provides moral guidance to its members.";

-- Institutional Moral Agents
CREATE moral_agents:government SET
  agent_name = "Government",
  agent_type = "Institution",
  description = "A governing body with authority and moral obligations.";

CREATE moral_agents:judicial_system SET
  agent_name = "Judicial System",
  agent_type = "Institution",
  description = "The legal system responsible for upholding justice and moral law.";

CREATE moral_agents:healthcare_system SET
  agent_name = "Healthcare System",
  agent_type = "Institution",
  description = "Medical institutions responsible for ethical healthcare delivery.";

CREATE moral_agents:media_organization SET
  agent_name = "Media Organization",
  agent_type = "Institution",
  description = "News and information providers with ethical reporting responsibilities.";

-- Artificial Intelligence Moral Agents
CREATE moral_agents:ai_system SET
  agent_name = "AI System",
  agent_type = "Artificial Intelligence",
  description = "An artificial intelligence system with decision-making capabilities.";

CREATE moral_agents:autonomous_robot SET
  agent_name = "Autonomous Robot",
  agent_type = "Artificial Intelligence",
  description = "A physical robot with independent decision-making abilities.";

CREATE moral_agents:ai_assistant SET
  agent_name = "AI Assistant",
  agent_type = "Artificial Intelligence",
  description = "An AI system designed to help humans make ethical decisions.";

-- Group Moral Agents
CREATE moral_agents:community SET
  agent_name = "Local Community",
  agent_type = "Group",
  description = "A group of individuals sharing common interests and moral responsibilities.";

CREATE moral_agents:family SET
  agent_name = "Family Unit",
  agent_type = "Group",
  description = "A family group with shared moral values and responsibilities.";

CREATE moral_agents:social_movement SET
  agent_name = "Social Movement",
  agent_type = "Group",
  description = "A collective working towards social change and moral progress.";

CREATE moral_agents:professional_association SET
  agent_name = "Professional Association",
  agent_type = "Group",
  description = "A group of professionals with shared ethical standards.";

-- Environmental Moral Agents
CREATE moral_agents:ecosystem SET
  agent_name = "Ecosystem",
  agent_type = "Environmental",
  description = "A natural system with intrinsic moral value.";

CREATE moral_agents:species SET
  agent_name = "Species",
  agent_type = "Environmental",
  description = "A biological group with moral consideration rights.";

CREATE moral_agents:future_generations SET
  agent_name = "Future Generations",
  agent_type = "Environmental",
  description = "Future human populations with moral rights to consideration."; 