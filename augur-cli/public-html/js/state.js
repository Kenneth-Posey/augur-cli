/**
 * state.js - Application state and DOM references
 *
 * The single `state` object holds all mutable application state:
 * navigation stack, parsed graph data, the Cytoscape instance, and
 * the currently displayed sidebar module. DOM references are cached
 * here as well.
 */
var state = {
    /** @type {Array<{level: string, id: string}>} Navigation breadcrumb stack */
    stack: [],
    /** @type {Object|null} Parsed graph-data.json */
    data: null,
    /** @type {cytoscape.Core|null} Cytoscape instance */
    cy: null,
    /** @type {string|null} Module id currently shown in the sidebar, or null */
    sidebarModule: null,
};

/* ---- Cached DOM references ---- */
var $cy = document.getElementById('cy');
var $status = document.getElementById('status');
var $breadcrumb = document.getElementById('breadcrumb');
var $sidebar = document.getElementById('sidebar');
var $backBtn = document.getElementById('back-btn');