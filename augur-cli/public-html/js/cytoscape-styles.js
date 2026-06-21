/**
 * cytoscape-styles.js — Cytoscape.js stylesheet
 *
 * Defines the visual style for all node types and edges in the
 * graph. Uses Cytoscape's JSON stylesheet syntax.
 */
var CY_STYLES = [
    {
        selector: 'node',
        style: {
            'background-color': '#0f3460',
            'label': 'data(label)',
            'color': '#e0e0e0',
            'font-size': '13px',
            'text-valign': 'center',
            'text-halign': 'center',
            'width': 'label',
            'height': 'label',
            'padding': '12px',
            'shape': 'round-rectangle',
            'border-width': 1,
            'border-color': '#1a4a8a',
        }
    },
    {
        selector: 'node[level="0"]',
        style: {
            'font-size': '15px',
            'font-weight': 'bold',
            'padding': '16px',
            'border-width': 2,
            'text-wrap': 'wrap',
            'text-max-width': '160px',
        }
    },
    {
        selector: 'node[level="1"][hasChildren="true"]',
        style: {
            'border-style': 'double',
            'border-color': '#4ecdc4',
            'border-width': 2,
        }
    },
    {
        selector: 'node.ghost',
        style: {
            'background-color': '#2a2a4e',
            'border-color': '#555',
            'border-width': 1,
            'border-style': 'dashed',
            'font-size': '11px',
            'color': '#8899aa',
            'padding': '6px',
            'shape': 'round-diamond',
            'width': 'label',
            'height': 'label',
            'text-wrap': 'wrap',
            'text-max-width': '120px',
        }
    },
    {
        selector: 'edge',
        style: {
            'curve-style': 'taxi',
            'taxi-direction': 'vertical',
            'target-arrow-shape': 'triangle',
            'target-arrow-color': '#555',
            'line-color': '#555',
            'width': 1.5,
            'arrow-scale': 0.8,
        }
    },
    {
        selector: 'edge.cross-crate',
        style: {
            'line-style': 'dashed',
            'line-color': '#888',
            'opacity': 0.5,
            'width': 1.2,
            'target-arrow-shape': 'none',
        }
    },
    {
        selector: 'node:selected',
        style: {
            'border-color': '#e94560',
            'border-width': 3,
        }
    },
    {
        selector: 'node:active',
        style: {
            'border-color': '#e94560',
            'border-width': 3,
        }
    },
    {
        selector: 'edge:active',
        style: {
            'z-index': 100,
            'line-color': '#e94560',
            'target-arrow-color': '#e94560',
            'width': 2.5,
        }
    },
    {
        selector: 'edge:selected',
        style: {
            'line-color': '#e94560',
            'target-arrow-color': '#e94560',
            'width': 2.5,
        }
    },
];