import * as fs from "fs";
import * as path from "path";

// --- Constants ---
const TOTAL_COMPONENTS = 10000;
const COMPONENTS_PER_MODULE = 15;
const TOTAL_MODULES = Math.ceil(TOTAL_COMPONENTS / COMPONENTS_PER_MODULE);
const FIXTURES_DIR = path.resolve(__dirname, "./fixtures");

// Common NPM packages to import from
const NPM_PACKAGES = [
    "react",
    "react-dom",
    "lodash",
    "classnames",
    "axios",
    "date-fns",
    "framer-motion",
    "react-query",
    "styled-components",
    "react-router-dom",
    "uuid",
    "moment",
    "ramda",
    "immutable",
    "rxjs",
];

// --- Utility Functions ---

/**
 * Shuffles an array and returns a new shuffled array
 */
function shuffle<T>(array: T[]): T[] {
    const shuffled = [...array];
    for (let i = shuffled.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    return shuffled;
}

/**
 * Gets a random selection of items from an array
 */
function getRandomItems<T>(array: T[], count: number): T[] {
    return shuffle(array).slice(0, count);
}

/**
 * Generates a component name based on module and component index
 */
function getComponentName(moduleIndex: number, componentIndex: number): string {
    const globalIndex = moduleIndex * COMPONENTS_PER_MODULE + componentIndex;
    return `Component${globalIndex.toString().padStart(5, "0")}`;
}

/**
 * Gets the module directory name
 */
function getModuleName(moduleIndex: number): string {
    return `module-${moduleIndex.toString().padStart(3, "0")}`;
}

/**
 * Generates imports for a component
 */
function generateImports(
    moduleIndex: number,
    componentIndex: number,
): {
    imports: string[];
    usedComponents: string[];
} {
    const imports: string[] = [];
    const usedComponents: string[] = [];

    // 1. Same-module imports (5 components from current module)
    const currentModuleComponents: string[] = [];
    for (let i = 0; i < COMPONENTS_PER_MODULE; i++) {
        if (i !== componentIndex) {
            currentModuleComponents.push(getComponentName(moduleIndex, i));
        }
    }
    const sameModuleImports = getRandomItems(currentModuleComponents, Math.min(5, currentModuleComponents.length));

    sameModuleImports.forEach((componentName) => {
        imports.push(`import { ${componentName} } from './${componentName}';`);
        usedComponents.push(componentName);
    });

    // 2. Cross-module barrel file imports (5 components from different modules)
    const otherModules = Array.from({ length: TOTAL_MODULES }, (_, i) => i).filter((i) => i !== moduleIndex);
    const selectedModules = getRandomItems(otherModules, Math.min(5, otherModules.length));

    selectedModules.forEach((targetModuleIndex) => {
        const targetComponentIndex = Math.floor(Math.random() * COMPONENTS_PER_MODULE);
        const componentName = getComponentName(targetModuleIndex, targetComponentIndex);
        const moduleName = getModuleName(targetModuleIndex);
        imports.push(`import { ${componentName} } from '../${moduleName}/index.ts';`);
        usedComponents.push(componentName);
    });

    // 3. Cross-module alias imports using #src/* (5 components from different modules)
    const aliasModules = getRandomItems(
        otherModules.filter((i) => !selectedModules.includes(i)),
        Math.min(5, otherModules.length - selectedModules.length),
    );

    aliasModules.forEach((targetModuleIndex) => {
        const targetComponentIndex = Math.floor(Math.random() * COMPONENTS_PER_MODULE);
        const componentName = getComponentName(targetModuleIndex, targetComponentIndex);
        const moduleName = getModuleName(targetModuleIndex);
        imports.push(`import { ${componentName} } from '#src/${moduleName}';`);
        usedComponents.push(componentName);
    });

    // 4. NPM package imports (5 random packages)
    const selectedPackages = getRandomItems(NPM_PACKAGES, 5);
    selectedPackages.forEach((packageName, index) => {
        // Generate different import patterns for variety
        switch (index % 4) {
            case 0:
                imports.push(`import ${packageName.replace(/[^a-zA-Z]/g, "")} from '${packageName}';`);
                break;
            case 1:
                imports.push(`import { ${packageName.replace(/[^a-zA-Z]/g, "")}Util } from '${packageName}';`);
                break;
            case 2:
                imports.push(`import * as ${packageName.replace(/[^a-zA-Z]/g, "")}Lib from '${packageName}';`);
                break;
            case 3:
                imports.push(
                    `import { default as ${packageName.replace(/[^a-zA-Z]/g, "")}Default } from '${packageName}';`,
                );
                break;
        }
    });

    return { imports, usedComponents };
}

/**
 * Generates the content of a React component
 */
function generateComponentContent(moduleIndex: number, componentIndex: number): string {
    const componentName = getComponentName(moduleIndex, componentIndex);
    const { imports, usedComponents } = generateImports(moduleIndex, componentIndex);

    // Generate some dummy props and state
    const props = ["title", "description", "isVisible", "onClick", "className"];
    const stateVars = ["isLoading", "data", "error", "count"];

    return `${imports.join("\n")}

interface ${componentName}Props {
  ${props.map((prop) => `${prop}?: any;`).join("\n  ")}
}

export const ${componentName}: React.FC<${componentName}Props> = ({
  ${props.join(",\n  ")}
}) => {
  // State management
  const [${stateVars.join(", ")}] = React.useState({
    ${stateVars.map((v) => `${v}: null`).join(",\n    ")}
  });

  // Effect hooks for demonstration
  React.useEffect(() => {
    // Simulate data fetching
    const fetchData = async () => {
      try {
        // Simulate API call
        await new Promise(resolve => setTimeout(resolve, 100));
        console.log('Data loaded for ${componentName}');
      } catch (err) {
        console.error('Error in ${componentName}:', err);
      }
    };

    fetchData();
  }, []);

  // Event handlers
  const handleClick = React.useCallback(() => {
    if (onClick) {
      onClick();
    }
    console.log('${componentName} clicked');
  }, [onClick]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    console.log('Form submitted in ${componentName}');
  };

  // Render logic with used components
  const renderContent = () => {
    if (isLoading) {
      return <div>Loading...</div>;
    }

    return (
      <div className={\`${componentName.toLowerCase()}-container \${className || ''}\`}>
        <h2>{title || '${componentName} Title'}</h2>
        <p>{description || 'Default description for ${componentName}'}</p>

        {/* Using imported components */}
        <div className="imported-components">
          ${usedComponents
              .slice(0, 10)
              .map((comp) => `<${comp} key="${comp}" />`)
              .join("\n          ")}
        </div>

        <form onSubmit={handleSubmit}>
          <input
            type="text"
            placeholder="Enter text for ${componentName}"
            onChange={(e) => console.log(e.target.value)}
          />
          <button type="submit">Submit</button>
        </form>

        <div className="actions">
          <button onClick={handleClick} disabled={!isVisible}>
            Click me in ${componentName}
          </button>
        </div>

        {/* Additional content to reach ~100 lines */}
        <div className="additional-content">
          <ul>
            {Array.from({ length: 5 }, (_, i) => (
              <li key={i}>Item {i + 1} in ${componentName}</li>
            ))}
          </ul>

          <div className="metrics">
            <span>Count: {count}</span>
            <span>Error: {error ? 'Yes' : 'No'}</span>
            <span>Data: {data ? 'Loaded' : 'Not loaded'}</span>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="${componentName.toLowerCase()}-wrapper">
      {renderContent()}
    </div>
  );
};

export default ${componentName};
`;
}

/**
 * Generates the barrel file (index.ts) content for a module
 */
function generateBarrelFileContent(moduleIndex: number): string {
    const exports: string[] = [];

    for (let i = 0; i < COMPONENTS_PER_MODULE; i++) {
        const componentName = getComponentName(moduleIndex, i);
        exports.push(`export { ${componentName} } from './${componentName}';`);
    }

    return exports.join("\n") + "\n";
}

/**
 * Creates directory if it doesn't exist
 */
function ensureDirectoryExists(dirPath: string): void {
    if (!fs.existsSync(dirPath)) {
        fs.mkdirSync(dirPath, { recursive: true });
    }
}

/**
 * Main function to generate all fixtures
 */
async function generateFixtures(): Promise<void> {
    console.log(`Generating ${TOTAL_COMPONENTS} components across ${TOTAL_MODULES} modules...`);
    console.log(`Each module will contain ${COMPONENTS_PER_MODULE} components.`);
    console.log(`Output directory: ${FIXTURES_DIR}`);

    // Clean and create fixtures directory
    if (fs.existsSync(FIXTURES_DIR)) {
        fs.rmSync(FIXTURES_DIR, { recursive: true, force: true });
    }
    ensureDirectoryExists(FIXTURES_DIR);

    // Generate modules and components
    for (let moduleIndex = 0; moduleIndex < TOTAL_MODULES; moduleIndex++) {
        const moduleName = getModuleName(moduleIndex);
        const moduleDir = path.join(FIXTURES_DIR, moduleName);

        console.log(`Generating module ${moduleIndex + 1}/${TOTAL_MODULES}: ${moduleName}`);
        ensureDirectoryExists(moduleDir);

        // Generate components for this module
        for (let componentIndex = 0; componentIndex < COMPONENTS_PER_MODULE; componentIndex++) {
            const componentName = getComponentName(moduleIndex, componentIndex);
            const componentContent = generateComponentContent(moduleIndex, componentIndex);
            const componentPath = path.join(moduleDir, `${componentName}.tsx`);

            fs.writeFileSync(componentPath, componentContent, "utf8");
        }

        // Generate barrel file for this module
        const barrelContent = generateBarrelFileContent(moduleIndex);
        const barrelPath = path.join(moduleDir, "index.ts");
        fs.writeFileSync(barrelPath, barrelContent, "utf8");
    }

    console.log("✅ Fixture generation completed!");
    console.log(`📁 Generated ${TOTAL_MODULES} modules with ${TOTAL_COMPONENTS} components`);
    console.log(`📄 Each component has ~20 imports (5 same-module, 5 cross-module barrel, 5 aliased, 5 NPM)`);
    console.log(`🎯 Ready for SWC plugin performance testing`);
}

generateFixtures().catch(console.error);
