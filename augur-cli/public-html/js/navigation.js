/**
 * navigation.js - Graph navigation (workspace / crate / submodule / leaf)
 *
 * Provides navigateTo(), navigateBack(), and renderGraph() which
 * handle the breadcrumb stack, Cytoscape element building, layout
 * execution, ghost node insertion, and graph fitting.
 */

function navigateTo(level, id) {
    closeSidebar();
    state.stack.push({ level: level, id: id });
    renderGraph(level, id);
    renderBreadcrumb();
    updateBackBtn();
}

function navigateBack() {
    if (state.stack.length <= 1) return;
    closeSidebar();
    state.stack.pop();
    var top = state.stack[state.stack.length - 1];
    renderGraph(top.level, top.id);
    renderBreadcrumb();
    updateBackBtn();
}

function renderGraph(level, id) {
    var elements;
    if (level === 'submodule') {
        var crateId = state.stack[state.stack.length - 2].id;
        elements = buildSubmoduleElements(crateId, id);
    } else {
        elements = buildElements(level, id);
    }
    var cy = state.cy;

    cy.elements().remove();
    cy.add(elements);

    if (level === 'workspace') {
        runTopDownLayout(cy, elements);
    } else if (level === 'crate' || level === 'submodule') {
        runTopDownLayout(cy, elements);
        if (level === 'crate') {
            addCrossCrateElements(id);
        }
        cy.fit(60);
    }
}