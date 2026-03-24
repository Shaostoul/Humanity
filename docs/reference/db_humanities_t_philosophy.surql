USE NS project_universe;
USE DB humanities;
  
DEFINE TABLE philosophy SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';
  
DEFINE FIELD name            ON TABLE philosophy TYPE string;
DEFINE FIELD description     ON TABLE philosophy TYPE string;
DEFINE FIELD type           ON TABLE philosophy TYPE string;
DEFINE FIELD principle      ON TABLE philosophy TYPE string;
DEFINE FIELD components     ON TABLE philosophy TYPE array;
DEFINE FIELD requirements   ON TABLE philosophy TYPE array;
DEFINE FIELD characteristics ON TABLE philosophy TYPE object;
DEFINE FIELD variations     ON TABLE philosophy TYPE array;
DEFINE FIELD applications   ON TABLE philosophy TYPE array;
DEFINE FIELD related_systems ON TABLE philosophy TYPE array;
  
DEFINE INDEX unique_philosophical_system ON TABLE philosophy COLUMNS name UNIQUE;
DEFINE INDEX philosophical_type ON TABLE philosophy COLUMNS type;
  
-- Ethics
CREATE philosophy:ethics SET
  name = "Ethical Framework",
  description = "System of moral principles and values",
  type = "Moral",
  principle = "Guidelines for right and wrong behavior",
  components = ["Moral principles", "Value systems", "Decision frameworks", "Behavioral guidelines"],
  requirements = ["Critical thinking", "Moral reasoning", "Cultural awareness", "Personal reflection"],
  characteristics = {
    universality: "Variable",
    adaptability: "High",
    complexity: "High",
    practicality: "High"
  },
  variations = ["Cultural ethics", "Professional ethics", "Personal ethics", "Applied ethics"],
  applications = ["Moral decision-making", "Social conduct", "Professional behavior"],
  related_systems = ["religion:religious_system", "society:social_structure"];
  
-- Epistemology
CREATE philosophy:epistemology SET
  name = "Knowledge Theory",
  description = "Study of knowledge and belief systems",
  type = "Epistemic",
  principle = "Understanding the nature and limits of knowledge",
  components = ["Knowledge theories", "Belief systems", "Truth criteria", "Learning methods"],
  requirements = ["Critical analysis", "Logical reasoning", "Evidence evaluation", "Open-mindedness"],
  characteristics = {
    rigor: "High",
    complexity: "Very high",
    applicability: "Broad",
    evolution: "Continuous"
  },
  variations = ["Scientific epistemology", "Religious epistemology", "Social epistemology", "Personal epistemology"],
  applications = ["Research methodology", "Education systems", "Knowledge acquisition"],
  related_systems = ["education:learning_system", "science:scientific_method"];
  
-- Metaphysics
CREATE philosophy:metaphysics SET
  name = "Reality Theory",
  description = "Study of fundamental nature of reality",
  type = "Metaphysical",
  principle = "Understanding the nature of existence and reality",
  components = ["Ontology", "Cosmology", "Causality", "Consciousness"],
  requirements = ["Abstract thinking", "Conceptual analysis", "Logical reasoning", "Open-mindedness"],
  characteristics = {
    abstraction: "High",
    complexity: "Very high",
    speculation: "High",
    relevance: "Fundamental"
  },
  variations = ["Materialism", "Idealism", "Dualism", "Process philosophy"],
  applications = ["Scientific theory", "Religious belief", "Personal worldview"],
  related_systems = ["science:physics", "religion:religious_system"];
  
-- Space Philosophy
CREATE philosophy:space_philosophy SET
  name = "Space Philosophy",
  description = "Philosophical framework for space exploration and colonization",
  type = "Applied",
  principle = "Ethical and philosophical considerations in space",
  components = ["Space ethics", "Extraterrestrial rights", "Space governance", "Human expansion"],
  requirements = ["Space knowledge", "Ethical reasoning", "Future thinking", "Global perspective"],
  characteristics = {
    novelty: "High",
    complexity: "Very high",
    urgency: "Growing",
    scope: "Universal"
  },
  variations = ["Space ethics", "Space governance", "Extraterrestrial philosophy", "Space humanism"],
  applications = ["Space policy", "Colonization ethics", "Extraterrestrial relations"],
  related_systems = ["society:space_community", "science:astronomy"]; 