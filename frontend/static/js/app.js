document.addEventListener('DOMContentLoaded', function() {
    // Handle choice buttons - SIMPLIFIED VERSION
    document.querySelectorAll('.choice-button').forEach(button => {
        button.addEventListener('click', async (e) => {
            e.preventDefault();
            
            const id = button.dataset.id;
            const nextPage = button.dataset.nextPage;
            
            // Immediate visual feedback
            button.style.transform = 'scale(0.95)';
            button.style.opacity = '0.7';
            
            // Determine API endpoint based on current page
            let apiEndpoint;
            if (window.location.pathname.includes('weapon')) {
                apiEndpoint = '/api/session/select_weapon';
            } else if (window.location.pathname.includes('land')) {
                apiEndpoint = '/api/session/select_land';
            } else if (window.location.pathname.includes('companion')) {
                apiEndpoint = '/api/session/select_companion';
            }
            
            if (apiEndpoint) {
                // Fire and forget API call - don't wait for response
                fetch(apiEndpoint, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ id: id })
                }).catch(err => console.warn('Selection API failed:', err));
            }
            
            // Navigate immediately - don't wait for API
            setTimeout(() => {
                window.location.href = nextPage;
            }, 100); // Minimal delay for visual feedback
        });
    });
    
    // Simplified image error handling
    document.querySelectorAll('.choice-image').forEach(img => {
        img.addEventListener('error', function() {
            this.style.background = '#ddd';
            this.alt = 'Image not available';
        });
    });
    
    // Handle start button
    const startButton = document.getElementById('startButton');
    if (startButton) {
        startButton.addEventListener('click', async function() {
            this.style.opacity = '0.7';
            
            // Fire and forget
            fetch('/api/session/start', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' }
            }).catch(err => console.warn('Start session API failed:', err));
            
            // Navigate immediately
            setTimeout(() => {
                window.location.href = '/entry/names';
            }, 100);
        });
    }
});
