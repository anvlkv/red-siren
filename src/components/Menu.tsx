function Menu({ setIsDark }: { setIsDark: (isDark: boolean) => void }) {
    return (
        <div>
            <h1>Menu</h1>
            <button onClick={() => setIsDark(true)}>Dark Mode</button>
        </div>
    );
}

export default Menu;
