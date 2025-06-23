// A helper function to make API calls for selections
async function handleSelection(endpoint, selectionId, nextPage) {
    try {
        const response = await fetch(endpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ id: selectionId })
        });

        if (response.ok) {
            window.location.href = nextPage;
        } else {
            alert(`Error: Could not save selection. Server responded with status ${response.status}.`);
        }
    } catch (error) {
        console.error(`Failed to post selection to ${endpoint}:`, error);
        alert('Failed to connect to the backend.');
    }
}


document.addEventListener('DOMContentLoaded', () => {
    // --- Start Page Logic ---
    const startButton = document.getElementById('startButton');
    if (startButton) {
        startButton.addEventListener('click', async () => {
            try {
                const response = await fetch('/api/session/start', { method: 'POST' });
                if (response.ok) {
                    const data = await response.json();
                    // Use the redirect URL from the server response
                    window.location.href = data.redirect || '/entry/names';
                } else {
                    alert('Error starting session.');
                }
            } catch (error) {
                console.error('Failed to start session:', error);
                alert('Failed to connect to the backend.');
            }
        });
    }

    // --- Name Entry Form Logic ---
    const nameForm = document.getElementById('nameForm');
    if (nameForm) {
        nameForm.addEventListener('submit', async (e) => {
            e.preventDefault();
            
            // Collect all non-empty names
            const names = [];
            for (let i = 1; i <= 5; i++) {
                const nameInput = document.getElementById(`name${i}`);
                if (nameInput && nameInput.value.trim()) {
                    names.push(nameInput.value.trim());
                }
            }
            
            if (names.length === 0) {
                alert('Please enter at least one name.');
                return;
            }
            
            try {
                const response = await fetch('/api/session/submit_names', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ names: names })
                });
                
                if (response.ok) {
                    // TODO: Navigate to next page (weapon select, etc.)
                    window.location.href = '/select/weapon';
                } else {
                    alert('Error submitting names.');
                }
            } catch (error) {
                console.error('Failed to submit names:', error);
                alert('Failed to connect to the backend.');
            }
        });
    }

    // --- Generic Choice Button Logic ---
    // This will work for weapon, land, and companion screens if they use the same class and data attributes.
    const choiceButtons = document.querySelectorAll('.choice-button');
    choiceButtons.forEach(button => {
        button.addEventListener('click', () => {
            const selectionId = button.dataset.id;
            const nextPage = button.dataset.nextPage;
            
            let apiEndpoint = '';
            if (selectionId.startsWith('weapon')) {
                apiEndpoint = '/api/session/select_weapon';
            } else if (selectionId.startsWith('land')) {
                apiEndpoint = '/api/session/select_land';
            } else if (selectionId.startsWith('companion')) {
                apiEndpoint = '/api/session/select_companion';
            }

            if (apiEndpoint) {
                handleSelection(apiEndpoint, selectionId, nextPage);
            }
        });
    });

    // TODO: Add event listeners for other pages like name entry, countdown, etc.
});
