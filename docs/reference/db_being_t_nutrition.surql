USE NS project_universe;
USE DB game;

DEFINE TABLE nutrition SCHEMAFULL
  PERMISSIONS
    FOR select WHERE true,
    FOR create, update, delete WHERE $auth.role = 'admin';

DEFINE FIELD id            ON TABLE nutrition TYPE string;
DEFINE FIELD being_id      ON TABLE nutrition TYPE record(beings);
DEFINE FIELD diet_type     ON TABLE nutrition TYPE string;
DEFINE FIELD calories      ON TABLE nutrition TYPE int;
DEFINE FIELD nutrients     ON TABLE nutrition TYPE object;
DEFINE FIELD restrictions  ON TABLE nutrition TYPE array;
DEFINE FIELD supplements   ON TABLE nutrition TYPE array;
DEFINE FIELD created_at    ON TABLE nutrition TYPE datetime;
DEFINE FIELD updated_at    ON TABLE nutrition TYPE datetime;

DEFINE INDEX unique_id ON TABLE nutrition COLUMNS id UNIQUE;

-- Example nutrition
CREATE nutrition:human_nutrition_001 SET
  id = "human_nutrition_001",
  being_id = beings:human_001,
  diet_type = "Balanced",
  calories = 2000,
  nutrients = {
    protein: 50,
    carbohydrates: 250,
    fats: 70,
    vitamins: ["A", "B", "C", "D"],
    minerals: ["Iron", "Calcium", "Magnesium"]
  },
  restrictions = ["Gluten-free"],
  supplements = ["Vitamin D", "Omega-3"],
  created_at = time::now(),
  updated_at = time::now();

CREATE nutrition:alien_nutrition_001 SET
  id = "alien_nutrition_001",
  being_id = beings:alien_001,
  diet_type = "Energy-based",
  calories = 3000,
  nutrients = {
    quantum_energy: 100,
    plasma: 50,
    minerals: ["Xylithium", "Crystal"],
    elements: ["Dark Matter"]
  },
  restrictions = ["Organic matter"],
  supplements = ["Quantum stabilizer"],
  created_at = time::now(),
  updated_at = time::now(); 