using System.Collections.Concurrent;
using System.Reflection;
using System.Security.Principal;
using System.Text.Json;
using System.Text.Json.Serialization;

const string Protocol = "dev.dscc.hidmaestro-broker.v1";

var broker = new HidMaestroBroker();
while (Console.ReadLine() is { } line)
{
    if (string.IsNullOrWhiteSpace(line)) continue;
    BrokerRequest? request;
    try
    {
        request = JsonSerializer.Deserialize<BrokerRequest>(line, JsonOptions.Default);
    }
    catch
    {
        continue;
    }

    if (request is null || request.Protocol != Protocol) continue;
    if (request.Command == "update")
    {
        broker.Update(request);
        continue;
    }

    var response = broker.Handle(request);
    Console.WriteLine(JsonSerializer.Serialize(response, JsonOptions.Default));
    Console.Out.Flush();
    if (request.Command == "shutdown") break;
}

sealed class HidMaestroBroker
{
    private readonly ConcurrentDictionary<string, HidMaestroSession> _sessions = new();
    private readonly object _loadLock = new();
    private HidMaestroRuntime? _runtime;
    private string? _loadError;

    public BrokerResponse Handle(BrokerRequest request)
    {
        try
        {
            return request.Command switch
            {
                "hello" => BrokerResponse.Success(request.Id, "DSCC HIDMaestro broker ready.", available: true, supportedKinds: ["xbox360"]),
                "provider_status" => ProviderStatus(request.Id),
                "create" => Create(request),
                "destroy" => Destroy(request),
                "cleanup" => Cleanup(request.Id),
                "shutdown" => Cleanup(request.Id),
                _ => BrokerResponse.Fail(request.Id, "Unknown broker command.")
            };
        }
        catch (Exception error)
        {
            BrokerLog.Error(error.ToString());
            return BrokerResponse.Fail(request.Id, "HIDMaestro broker command failed.");
        }
    }

    public void Update(BrokerRequest request)
    {
        if (request.SessionId is null || request.State is null) return;
        if (_sessions.TryGetValue(request.SessionId, out var session))
        {
            session.Submit(request.State);
        }
    }

    private BrokerResponse ProviderStatus(ulong id)
    {
        if (!BrokerSecurity.IsElevated())
        {
            return BrokerResponse.Success(id, "HIDMaestro broker requires administrator privileges.", available: false);
        }
        var runtime = Runtime();
        if (runtime is null)
        {
            return BrokerResponse.Success(id, _loadError ?? "HIDMaestro.Core.dll is unavailable.", available: false);
        }
        return BrokerResponse.Success(id, "HIDMaestro broker is available.", available: true, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Create(BrokerRequest request)
    {
        if (!BrokerSecurity.IsElevated())
        {
            return BrokerResponse.Fail(request.Id, "HIDMaestro broker requires administrator privileges.");
        }
        if (request.ControllerId is null || request.Kind != "xbox360")
        {
            return BrokerResponse.Fail(request.Id, "Unsupported virtual output request.");
        }
        var runtime = Runtime();
        if (runtime is null)
        {
            return BrokerResponse.Fail(request.Id, _loadError ?? "HIDMaestro runtime unavailable.");
        }
        var sessionId = $"dscc-{Guid.NewGuid():N}";
        _sessions[sessionId] = runtime.CreateXbox360Session();
        return BrokerResponse.Success(request.Id, "HIDMaestro virtual controller created.", sessionId: sessionId, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Destroy(BrokerRequest request)
    {
        if (request.SessionId is not null && _sessions.TryRemove(request.SessionId, out var session))
        {
            session.Dispose();
        }
        return BrokerResponse.Success(request.Id, "HIDMaestro virtual controller destroyed.", available: true, supportedKinds: ["xbox360"]);
    }

    private BrokerResponse Cleanup(ulong id)
    {
        foreach (var session in _sessions.Values) session.Dispose();
        _sessions.Clear();
        return BrokerResponse.Success(id, "HIDMaestro broker cleanup complete.", available: true, supportedKinds: ["xbox360"]);
    }

    private HidMaestroRuntime? Runtime()
    {
        if (_runtime is not null || _loadError is not null) return _runtime;
        lock (_loadLock)
        {
            if (_runtime is not null || _loadError is not null) return _runtime;
            try
            {
                _runtime = HidMaestroRuntime.Load();
            }
            catch (Exception error)
            {
                BrokerLog.Error(error.ToString());
                _loadError = error.Message;
            }
            return _runtime;
        }
    }
}

static class BrokerLog
{
    public static void Error(string message)
    {
        if (Environment.GetEnvironmentVariable("DSCC_HIDMAESTRO_BROKER_DEBUG") == "1")
        {
            Console.Error.WriteLine(message);
        }
    }
}

static class BrokerSecurity
{
    public static bool IsElevated()
    {
        if (!OperatingSystem.IsWindows()) return false;
        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }
}

sealed class HidMaestroRuntime
{
    private readonly object _context;
    private readonly MethodInfo _getProfile;
    private readonly MethodInfo _createController;
    private readonly MethodInfo? _catalogProfileById;
    private readonly Type _stateType;
    private readonly Type _buttonType;
    private readonly Type _hatType;
    private readonly Type _helpersType;

    private HidMaestroRuntime(
        object context,
        MethodInfo getProfile,
        MethodInfo createController,
        MethodInfo? catalogProfileById,
        Type stateType,
        Type buttonType,
        Type hatType,
        Type helpersType)
    {
        _context = context;
        _getProfile = getProfile;
        _createController = createController;
        _catalogProfileById = catalogProfileById;
        _stateType = stateType;
        _buttonType = buttonType;
        _hatType = hatType;
        _helpersType = helpersType;
    }

    public static HidMaestroRuntime Load()
    {
        var baseDir = AppContext.BaseDirectory;
        var assemblyPath = Path.Combine(baseDir, "HIDMaestro.Core.dll");
        if (!File.Exists(assemblyPath)) throw new InvalidOperationException("HIDMaestro.Core.dll was not found next to the broker.");
        var assembly = Assembly.LoadFrom(assemblyPath);
        var contextType = FindType(assembly, "HMContext");
        var stateType = FindType(assembly, "HMGamepadState");
        var buttonType = FindType(assembly, "HMButton");
        var hatType = FindType(assembly, "HMHat");
        var helpersType = FindType(assembly, "HMGamepadStateHelpers");
        var catalogType = assembly.GetTypes().FirstOrDefault(type => type.Name == "HMaestroProfileCatalog");
        var context = CreateContext(contextType);
        contextType.GetMethod("LoadDefaultProfiles", BindingFlags.Public | BindingFlags.Instance, Type.EmptyTypes)?.Invoke(context, []);
        var getProfile = RequiredMethod(contextType, "GetProfile");
        var createController = RequiredMethod(contextType, "CreateController");
        var catalogProfileById = catalogType?.GetMethods(BindingFlags.Public | BindingFlags.Static)
            .FirstOrDefault(method => method.Name == "GetProfileById");
        return new HidMaestroRuntime(context, getProfile, createController, catalogProfileById, stateType, buttonType, hatType, helpersType);
    }

    public HidMaestroSession CreateXbox360Session()
    {
        var profile = _getProfile.Invoke(_context, ["xbox-360-wired"])
            ?? _catalogProfileById?.Invoke(null, ["xbox-360-wired"])
            ?? throw new InvalidOperationException("HIDMaestro Xbox 360 profile was not found.");
        var controller = _createController.Invoke(_context, [profile])
            ?? throw new InvalidOperationException("HIDMaestro controller creation failed.");
        controller.GetType().GetMethod("Connect", Type.EmptyTypes)?.Invoke(controller, []);
        return new HidMaestroSession(controller, profile, _stateType, _buttonType, _hatType, _helpersType);
    }

    private static object CreateContext(Type contextType)
    {
        var ctor = contextType.GetConstructor(Type.EmptyTypes);
        if (ctor is not null) return ctor.Invoke([]);
        foreach (var name in new[] { "Create", "Open", "Default" })
        {
            var method = contextType.GetMethod(name, BindingFlags.Public | BindingFlags.Static, Type.EmptyTypes);
            if (method?.Invoke(null, []) is { } context) return context;
        }
        throw new InvalidOperationException("HIDMaestro context factory was not found.");
    }

    private static Type FindType(Assembly assembly, string name) =>
        assembly.GetTypes().FirstOrDefault(type => type.Name == name)
        ?? throw new InvalidOperationException($"HIDMaestro type {name} was not found.");

    private static MethodInfo RequiredMethod(Type type, string name) =>
        type.GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.Static)
            .FirstOrDefault(method => method.Name == name)
        ?? throw new InvalidOperationException($"HIDMaestro method {name} was not found.");
}

sealed class HidMaestroSession : IDisposable
{
    private readonly object _controller;
    private readonly object _profile;
    private readonly Type _stateType;
    private readonly Type _buttonType;
    private readonly Type _hatType;
    private readonly Type _helpersType;
    private readonly MethodInfo _submitState;

    public HidMaestroSession(object controller, object profile, Type stateType, Type buttonType, Type hatType, Type helpersType)
    {
        _controller = controller;
        _profile = profile;
        _stateType = stateType;
        _buttonType = buttonType;
        _hatType = hatType;
        _helpersType = helpersType;
        _submitState = controller.GetType().GetMethod("SubmitState")
            ?? throw new InvalidOperationException("HIDMaestro SubmitState method was not found.");
    }

    public void Submit(VirtualGamepadStateDto state)
    {
        var hmState = Activator.CreateInstance(_stateType)
            ?? throw new InvalidOperationException("HIDMaestro state creation failed.");
        var buttons = state.Buttons?.Buttons;
        SetMember(hmState, "Axes", StandardAxes(state));
        SetMember(hmState, "Buttons", Buttons(buttons));
        SetMember(hmState, "Hat", Hat(buttons));
        _submitState.Invoke(_controller, [hmState]);
    }

    public void Dispose()
    {
        try { _controller.GetType().GetMethod("Disconnect", Type.EmptyTypes)?.Invoke(_controller, []); } catch { }
        if (_controller is IDisposable disposable) disposable.Dispose();
    }

    private object? StandardAxes(VirtualGamepadStateDto state)
    {
        var method = _helpersType.GetMethods(BindingFlags.Public | BindingFlags.Static)
            .FirstOrDefault(candidate => candidate.Name == "StandardAxes" && candidate.GetParameters().Length >= 7);
        if (method is null) return null;
        return method.Invoke(null, [
            _profile,
            SignedToUnit(state.LeftStick?.X),
            SignedToUnit(state.LeftStick?.Y),
            SignedToUnit(state.RightStick?.X),
            SignedToUnit(state.RightStick?.Y),
            Unit(state.Triggers?.L2),
            Unit(state.Triggers?.R2)
        ]);
    }

    private object Buttons(Dictionary<string, bool>? buttons)
    {
        ulong value = 0;
        foreach (var (key, pressed) in buttons ?? new Dictionary<string, bool>())
        {
            if (!pressed) continue;
            foreach (var name in ButtonNames(key))
            {
                try
                {
                    value |= Convert.ToUInt64(Enum.Parse(_buttonType, name, ignoreCase: true));
                    break;
                }
                catch { }
            }
        }
        return Enum.ToObject(_buttonType, value);
    }

    private object Hat(Dictionary<string, bool>? buttons)
    {
        var up = ButtonPressed(buttons, "dpad_up");
        var right = ButtonPressed(buttons, "dpad_right");
        var down = ButtonPressed(buttons, "dpad_down");
        var left = ButtonPressed(buttons, "dpad_left");

        if (up == down)
        {
            up = false;
            down = false;
        }
        if (left == right)
        {
            left = false;
            right = false;
        }

        var name = (up, right, down, left) switch
        {
            (true, true, false, false) => "NorthEast",
            (true, false, false, true) => "NorthWest",
            (false, true, true, false) => "SouthEast",
            (false, false, true, true) => "SouthWest",
            (true, false, false, false) => "North",
            (false, true, false, false) => "East",
            (false, false, true, false) => "South",
            (false, false, false, true) => "West",
            _ => "None"
        };
        return Enum.Parse(_hatType, name, ignoreCase: true);
    }

    private static IEnumerable<string> ButtonNames(string key) => key switch
    {
        "a" => ["A"],
        "b" => ["B"],
        "x" => ["X"],
        "y" => ["Y"],
        "dpad_up" => ["DpadUp", "DPadUp", "Up"],
        "dpad_down" => ["DpadDown", "DPadDown", "Down"],
        "dpad_left" => ["DpadLeft", "DPadLeft", "Left"],
        "dpad_right" => ["DpadRight", "DPadRight", "Right"],
        "left_shoulder" => ["LeftBumper", "LeftShoulder", "LB"],
        "right_shoulder" => ["RightBumper", "RightShoulder", "RB"],
        "left_thumb" => ["LeftStick", "LeftThumb", "LS"],
        "right_thumb" => ["RightStick", "RightThumb", "RS"],
        "back" => ["Back", "Select"],
        "start" => ["Start"],
        "guide" => ["Guide", "Home"],
        "touchpad" => ["Touchpad"],
        "share" => ["Share", "Back"],
        _ => []
    };

    private static void SetMember(object target, string name, object? value)
    {
        if (value is null) return;
        var type = target.GetType();
        var property = type.GetProperty(name, BindingFlags.Public | BindingFlags.Instance);
        if (property is not null)
        {
            property.SetValue(target, value);
            return;
        }
        type.GetField(name, BindingFlags.Public | BindingFlags.Instance)?.SetValue(target, value);
    }

    private static bool ButtonPressed(Dictionary<string, bool>? buttons, string key) =>
        buttons is not null && buttons.TryGetValue(key, out var pressed) && pressed;

    private static float Unit(double? value) => (float)Math.Clamp(value ?? 0.0, 0.0, 1.0);
    private static float SignedToUnit(double? value) => (float)((Math.Clamp(value ?? 0.0, -1.0, 1.0) + 1.0) * 0.5);
}

sealed record BrokerRequest(
    string Protocol,
    ulong Id,
    string Command,
    string? ControllerId,
    string? SessionId,
    string? Kind,
    VirtualGamepadStateDto? State);

sealed record BrokerResponse(
    ulong Id,
    bool Ok,
    bool? Available,
    string? Message,
    string? SessionId,
    string[] SupportedKinds)
{
    public static BrokerResponse Success(
        ulong id,
        string message,
        bool? available = null,
        string? sessionId = null,
        string[]? supportedKinds = null) =>
        new(id, true, available, message, sessionId, supportedKinds ?? []);

    public static BrokerResponse Fail(ulong id, string message) =>
        new(id, false, false, message, null, []);
}

sealed record VirtualGamepadStateDto(
    VirtualStickDto? LeftStick,
    VirtualStickDto? RightStick,
    VirtualTriggerDto? Triggers,
    VirtualButtonDto? Buttons);

sealed record VirtualStickDto(double X, double Y);
sealed record VirtualTriggerDto(double L2, double R2);
sealed record VirtualButtonDto(Dictionary<string, bool> Buttons);

static class JsonOptions
{
    public static readonly JsonSerializerOptions Default = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        PropertyNameCaseInsensitive = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };
}
